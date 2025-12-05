use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use async_trait::async_trait;
use axum::RequestExt;
use axum::body::Body;
use axum::extract::{FromRequestParts, Query};
use axum::response::IntoResponse;
use http::header::HOST;
use http::request::Parts;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower::Service;
use url::Url;

use crate::client::{Client, ClientConfig};
use crate::error::{InEndpoint, InSource, Result, into_http};
use crate::feed::Feed;
use crate::filter::{FilterContext, FilterSkip};
use crate::filter_pipeline::{FilterPipeline, FilterPipelineConfig};
use crate::otf_filter::{OnTheFlyFilter, OnTheFlyFilterQuery};
use crate::source::{Source, SourceConfig};

type Request = http::Request<Body>;
type Response = http::Response<Body>;

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct EndpointConfig {
  pub path: Arc<str>,
  pub note: Option<String>,
  #[serde(flatten)]
  pub config: EndpointServiceConfig,
}

impl EndpointConfig {
  pub fn default_on_the_fly(path: &str) -> Self {
    Self {
      path: Arc::from(path),
      note: Some("Default On-the-fly filter endpoint".to_string()),
      config: EndpointServiceConfig {
        source: SourceConfig::Dynamic,
        filters: FilterPipelineConfig::default(),
        on_the_fly_filters: true,
        client: None,
      },
    }
  }

  pub fn path_sans_slash(&self) -> &str {
    self.path.strip_prefix('/').unwrap_or(&self.path)
  }

  #[cfg(test)]
  pub fn parse_yaml(yaml: &str) -> Result<Self> {
    Ok(serde_yaml::from_str(yaml)?)
  }

  pub async fn build(self) -> Result<EndpointService> {
    EndpointService::from_config(self.config, self.path.clone())
      .await
      .context(InEndpoint(self.path))
  }

  pub(crate) fn source(&self) -> &SourceConfig {
    &self.config.source
  }
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct EndpointServiceConfig {
  #[serde(default)]
  pub source: SourceConfig,
  #[serde(default)]
  pub filters: FilterPipelineConfig,
  #[serde(default)]
  pub on_the_fly_filters: bool,
  #[serde(default)]
  pub client: Option<ClientConfig>,
}

// Ideally I would implement this endpoint service to include a
// RequestContext field, and make an separate type that implements
// MakeService<http::Request, Response=EndpointService>. But axum
// Router doesn't support nest_make_service yet, so I will just
// approximate it by making request_context part of the Service input.
//
// This type should be kept cheap to clone. It will be cloned for each
// request.
#[derive(Clone)]
pub struct EndpointService {
  path: Arc<str>,
  // used for detecting changes in the config for partial update
  config: EndpointServiceConfig,
  source: Source,
  on_the_fly_filter: Option<Arc<Mutex<OnTheFlyFilter>>>,
  filters: Arc<FilterPipeline>,
  client: Arc<Client>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct EndpointParam {
  #[serde(default)]
  source: Option<Url>,
  /// Only process the initial N filter steps
  #[serde(default)]
  filter_skip: Option<FilterSkip>,
  /// Limit the number of items in the feed
  #[serde(default)]
  limit_posts: Option<usize>,
  /// The url base of the feed, used for resolving relative urls
  #[serde(skip)]
  base: Option<Url>,
  /// The full query string
  #[serde(skip)]
  query: Option<String>,
  /// Extra query parameters
  #[serde(flatten)]
  extra_queries: HashMap<String, String>,
}

impl EndpointParam {
  pub fn source(&self) -> Option<&Url> {
    self.source.as_ref()
  }

  pub const fn all_fields() -> &'static [&'static str] {
    &["source", "filter_skip", "limit_posts"]
  }

  pub(crate) fn base(&self) -> Option<&Url> {
    self.base.as_ref()
  }

  pub(crate) fn filter_skip(&self) -> Option<&FilterSkip> {
    self.filter_skip.as_ref()
  }

  pub(crate) fn extra_queries(&self) -> &HashMap<String, String> {
    &self.extra_queries
  }
}

#[async_trait]
impl<S> FromRequestParts<S> for EndpointParam
where
  S: Send + Sync,
{
  type Rejection = Infallible;

  async fn from_request_parts(
    parts: &mut Parts,
    state: &S,
  ) -> Result<Self, Self::Rejection> {
    let Query(mut param) = Query::<Self>::from_request_parts(parts, state)
      .await
      .unwrap_or_default();

    let query = parts.uri.query().map(std::string::ToString::to_string);

    param.base = Self::get_base(parts);
    param.query = query;

    Ok(param)
  }
}

impl EndpointParam {
  pub fn new(
    source: Option<Url>,
    filter_skip: Option<usize>,
    limit_posts: Option<usize>,
    base: Option<Url>,
  ) -> Self {
    Self {
      source,
      filter_skip: filter_skip.map(FilterSkip::upto),
      limit_posts,
      base,
      query: None,
      extra_queries: HashMap::new(),
    }
  }

  fn get_base(req: &Parts) -> Option<Url> {
    if let Some(url) = crate::util::app_base_from_env().as_ref() {
      return Some(url.clone());
    }

    if let Some(url) = Self::base_from_reverse_proxy(req) {
      return Some(url);
    }

    Self::base_from_host(req)
  }

  fn base_from_reverse_proxy(req: &Parts) -> Option<Url> {
    let host = req
      .headers
      .get("X-Forwarded-Host")
      .and_then(|x| x.to_str().ok())?;

    let proto = req
      .headers
      .get("X-Forwarded-Proto")
      .and_then(|x| x.to_str().ok())
      .unwrap_or("http");

    let base = format!("{proto}://{host}/");
    let base = base.parse().ok()?;
    Some(base)
  }

  fn base_from_host(req: &Parts) -> Option<Url> {
    let host = req.headers.get(HOST)?.to_str().ok()?;
    let base = format!("http://{host}/");
    let base = base.parse().ok()?;
    Some(base)
  }
}

impl Service<Request> for EndpointService {
  type Response = Response;
  type Error = Infallible;
  // not Sync because the request's Body may not be sync
  type Future =
    Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

  fn poll_ready(
    &mut self,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), Self::Error>> {
    std::task::Poll::Ready(Ok(()))
  }

  fn call(&mut self, req: Request) -> Self::Future {
    let this = self.clone();
    let fut = async move { Ok(this.handle(req).await.into_response()) };
    Box::pin(fut)
  }
}

impl EndpointService {
  /// Used in tests for replacing the client with a mock
  #[cfg(test)]
  pub fn with_client(mut self, client: Client) -> Self {
    self.client = Arc::new(client);
    self
  }

  pub fn source(&self) -> &Source {
    &self.source
  }

  pub fn config(&self) -> &EndpointServiceConfig {
    &self.config
  }

  async fn handle(self, mut req: Request) -> Result<Response, Response> {
    // infallible
    let param: EndpointParam = req.extract_parts().await.unwrap();
    let path = self.path.clone();
    let feed = self
      .run(param)
      .await
      .context(InEndpoint(path))
      .map_err(|e| into_http(e).into_response())?;
    let resp = feed.into_response();
    Ok(resp)
  }

  pub async fn from_config(
    config: EndpointServiceConfig,
    path: Arc<str>,
  ) -> Result<Self> {
    let cloned_config = config.clone();
    let filters = config.filters.build().await?;

    let default_cache_ttl = Duration::from_secs(15 * 60);
    let client = config.client.unwrap_or_default().build(default_cache_ttl)?;
    let source = config.source.try_into()?;
    let on_the_fly_filter = if config.on_the_fly_filters {
      Some(Default::default())
    } else {
      None
    };

    Ok(Self {
      path,
      config: cloned_config,
      source,
      on_the_fly_filter,
      filters: Arc::new(filters),
      client: Arc::new(client),
    })
  }

  pub async fn run_with_context(
    self,
    context: &mut FilterContext,
    param: EndpointParam,
  ) -> Result<Feed> {
    use anyhow::Context;
    let feed = self
      .source
      .fetch_feed(context, Some(&self.client))
      .await
      .context(InSource(self.source))?;
    let mut feed = self.filters.run(context, feed).await?;

    if let (Some(on_the_fly_filter), Some(query)) =
      (self.on_the_fly_filter, param.query)
    {
      let query = OnTheFlyFilterQuery::from_uri_query(&query);
      let mut lock = on_the_fly_filter.lock().await;
      feed = lock
        .run(query, context, feed)
        .await
        .context("error running otf filters")?;
    }

    if let Some(limit) = param.limit_posts {
      let mut posts = feed.take_posts();
      posts.truncate(limit);
      feed.set_posts(posts);
    }

    Ok(feed)
  }

  pub async fn run(self, param: EndpointParam) -> Result<Feed> {
    let mut context = FilterContext::from_param(&param);
    let feed = self.run_with_context(&mut context, param).await?;
    Ok(feed)
  }

  pub fn config_changed(&self, config: &EndpointServiceConfig) -> bool {
    self.config != *config
  }

  pub async fn update(mut self, config: EndpointServiceConfig) -> Result<Self> {
    let cloned_config = config.clone();
    if self.config.client != config.client {
      let default_cache_ttl = Duration::from_secs(15 * 60);
      let client =
        config.client.unwrap_or_default().build(default_cache_ttl)?;
      self.client = Arc::new(client);
    }

    if self.config.source != config.source {
      let source = config.source.try_into()?;
      self.source = source;
    }

    if self.config.filters != config.filters {
      self.filters.update(config.filters).await?;
    }

    self.config = cloned_config;

    Ok(self)
  }
}
