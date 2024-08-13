use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::{FromRequestParts, Query};
use axum::response::IntoResponse;
use axum::RequestExt;
use http::header::HOST;
use http::request::Parts;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower::Service;
use url::Url;

use crate::client::{Client, ClientConfig};
use crate::feed::Feed;
use crate::filter::FilterContext;
use crate::filter_pipeline::{FilterPipeline, FilterPipelineConfig};
use crate::otf_filter::{OnTheFlyFilter, OnTheFlyFilterQuery};
use crate::source::{Source, SourceConfig};
use crate::util::{ConfigError, Error, Result};

type Request = http::Request<Body>;
type Response = http::Response<Body>;

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct EndpointConfig {
  pub path: String,
  pub note: Option<String>,
  #[serde(flatten)]
  pub config: EndpointServiceConfig,
}

impl EndpointConfig {
  pub fn default_on_the_fly(path: &str) -> Self {
    Self {
      path: path.to_string(),
      note: Some("Default On-the-fly filter endpoint".to_string()),
      config: EndpointServiceConfig {
        source: None,
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
  pub fn parse_yaml(yaml: &str) -> Result<Self, ConfigError> {
    Ok(serde_yaml::from_str(yaml)?)
  }

  pub async fn build(self) -> Result<EndpointService, ConfigError> {
    EndpointService::from_config(self.config).await
  }

  pub(crate) fn source(&self) -> Option<&SourceConfig> {
    self.config.source.as_ref()
  }
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct EndpointServiceConfig {
  #[serde(default)]
  source: Option<SourceConfig>,
  #[serde(default)]
  filters: FilterPipelineConfig,
  #[serde(default)]
  on_the_fly_filters: bool,
  #[serde(default)]
  client: Option<ClientConfig>,
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
  // used for detecting changes in the config for partial update
  config: EndpointServiceConfig,
  source: Option<Source>,
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
  limit_filters: Option<usize>,
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
  pub const fn all_fields() -> &'static [&'static str] {
    &["source", "limit_filters", "limit_posts"]
  }

  pub(crate) fn base(&self) -> Option<&Url> {
    self.base.as_ref()
  }

  pub(crate) fn limit_filters(&self) -> Option<usize> {
    self.limit_filters
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

    let query = parts.uri.query().map(|q| q.to_string());

    param.base = Self::get_base(parts);
    param.query = query;

    Ok(param)
  }
}

impl EndpointParam {
  pub fn new(
    source: Option<Url>,
    limit_filters: Option<usize>,
    limit_posts: Option<usize>,
    base: Option<Url>,
  ) -> Self {
    Self {
      source,
      limit_filters,
      limit_posts,
      base,
      query: None,
      extra_queries: HashMap::new(),
    }
  }

  fn get_base(req: &Parts) -> Option<Url> {
    let host = req
      .headers
      .get("X-Forwarded-Host")
      .or_else(|| req.headers.get(HOST))
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

  pub fn source(&self) -> &Option<Source> {
    &self.source
  }

  async fn handle(self, mut req: Request) -> Result<Response, Response> {
    // infallible
    let param: EndpointParam = req.extract_parts().await.unwrap();
    let feed = self
      .run(param)
      .await
      .map_err(|e| e.into_http().into_response())?;
    let resp = feed.into_response();
    Ok(resp)
  }

  pub async fn from_config(
    config: EndpointServiceConfig,
  ) -> Result<Self, ConfigError> {
    let cloned_config = config.clone();
    let filters = config.filters.build().await?;

    let default_cache_ttl = Duration::from_secs(15 * 60);
    let client = config.client.unwrap_or_default().build(default_cache_ttl)?;
    let source = config.source.map(|s| s.try_into()).transpose()?;
    let on_the_fly_filter = if config.on_the_fly_filters {
      Some(Default::default())
    } else {
      None
    };

    Ok(Self {
      config: cloned_config,
      source,
      on_the_fly_filter,
      filters: Arc::new(filters),
      client: Arc::new(client),
    })
  }

  pub async fn run(self, param: EndpointParam) -> Result<Feed> {
    let source = self.find_source(&param.source)?;
    let mut context = FilterContext::from_param(&param);
    let feed = source
      .fetch_feed(&context, Some(&self.client))
      .await
      .map_err(|e| Error::FetchSource(Box::new(e)))?;
    if let Some(limit_filters) = param.limit_filters {
      context.set_limit_filters(limit_filters);
    }
    if let Some(base) = param.base {
      context.set_base(base);
    }
    // TODO: change filter pipeline to operate on a borrowed context
    let mut feed = self.filters.run(context.clone(), feed).await?;

    if let (Some(on_the_fly_filter), Some(query)) =
      (self.on_the_fly_filter, param.query)
    {
      let query = OnTheFlyFilterQuery::from_uri_query(&query);
      let mut lock = on_the_fly_filter.lock().await;
      feed = lock.run(query, context, feed).await?;
    }

    if let Some(limit) = param.limit_posts {
      let mut posts = feed.take_posts();
      posts.truncate(limit);
      feed.set_posts(posts);
    }

    Ok(feed)
  }

  fn find_source(&self, param: &Option<Url>) -> Result<Source> {
    match &self.source {
      // ignore the source from param if it's already specified in config
      Some(source) => Ok(source.clone()),
      None => param
        .as_ref()
        .ok_or(Error::Message("missing source".into()))
        .cloned()
        .map(Source::from),
    }
  }

  pub fn config_changed(&self, config: &EndpointServiceConfig) -> bool {
    self.config != *config
  }

  pub async fn update(
    mut self,
    config: EndpointServiceConfig,
  ) -> Result<Self, ConfigError> {
    let cloned_config = config.clone();
    if self.config.client != config.client {
      let default_cache_ttl = Duration::from_secs(15 * 60);
      let client =
        config.client.unwrap_or_default().build(default_cache_ttl)?;
      self.client = Arc::new(client);
    }

    if self.config.source != config.source {
      let source = config.source.map(|s| s.try_into()).transpose()?;
      self.source = source;
    }

    if self.config.filters != config.filters {
      self.filters.update(config.filters).await?;
    }

    self.config = cloned_config;

    Ok(self)
  }
}
