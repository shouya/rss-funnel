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
use http::StatusCode;
use mime::Mime;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tower::Service;
use url::Url;

use crate::client::{Client, ClientConfig};
use crate::filter::FilterContext;
use crate::filter_pipeline::{FilterPipeline, FilterPipelineConfig};
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
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct EndpointServiceConfig {
  #[serde(default)]
  source: Option<SourceConfig>,
  filters: FilterPipelineConfig,
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
  #[serde(
    default,
    alias = "pp",
    deserialize_with = "deserialize_bool_in_query"
  )]
  pretty_print: bool,
  /// The url base of the feed, used for resolving relative urls
  #[serde(skip)]
  base: Option<Url>,
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

    param.base = Self::get_base(parts);
    dbg!(&param);
    Ok(param)
  }
}

impl EndpointParam {
  pub fn new(
    source: Option<Url>,
    limit_filters: Option<usize>,
    limit_posts: Option<usize>,
    pretty_print: bool,
    base: Option<Url>,
  ) -> Self {
    Self {
      source,
      limit_filters,
      limit_posts,
      pretty_print,
      base,
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

#[derive(Clone)]
pub struct EndpointOutcome {
  feed_xml: String,
  content_type: Mime,
}

impl EndpointOutcome {
  pub fn new(feed_xml: String, content_type: &str) -> Self {
    let content_type = content_type.parse().expect("invalid content_type");

    Self {
      feed_xml,
      content_type,
    }
  }

  pub fn prettify(&mut self) {
    if let Ok(xml) = self.feed_xml.parse::<xmlem::Document>() {
      self.feed_xml = xml.to_string_pretty();
    }
  }

  pub fn feed_xml(&self) -> &str {
    &self.feed_xml
  }
}

impl IntoResponse for EndpointOutcome {
  fn into_response(self) -> axum::response::Response {
    let mut resp = Response::new(Body::from(self.feed_xml));
    resp.headers_mut().insert(
      "content-type",
      http::header::HeaderValue::from_str(self.content_type.as_ref())
        .expect("invalid content_type"),
    );
    resp
  }
}

impl Service<EndpointParam> for EndpointService {
  type Response = EndpointOutcome;
  type Error = Error;
  type Future =
    Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn poll_ready(
    &mut self,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), Self::Error>> {
    std::task::Poll::Ready(Ok(()))
  }

  fn call(&mut self, input: EndpointParam) -> Self::Future {
    let req = input;
    let this = self.clone();
    let fut = async { this.call_internal(req).await };
    Box::pin(fut)
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
    Service::<EndpointParam>::poll_ready(self, _cx).map_err(|_| unreachable!())
  }

  fn call(&mut self, mut req: Request) -> Self::Future {
    let this = self.clone();
    let fut = async move {
      let param = req.extract_parts().await.unwrap();
      this.call_internal(param).await
    };
    let fut = async {
      let err = |e: Error| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
      };
      let resp = fut.await.map(|x| x.into_response()).unwrap_or_else(err);
      Ok(resp)
    };
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

  pub async fn from_config(
    config: EndpointServiceConfig,
  ) -> Result<Self, ConfigError> {
    let cloned_config = config.clone();
    let filters = config.filters.build().await?;

    let default_cache_ttl = Duration::from_secs(15 * 60);
    let client = config.client.unwrap_or_default().build(default_cache_ttl)?;
    let source = config.source.map(|s| s.try_into()).transpose()?;

    Ok(Self {
      config: cloned_config,
      source,
      filters: Arc::new(filters),
      client: Arc::new(client),
    })
  }

  async fn call_internal(
    self,
    param: EndpointParam,
  ) -> Result<EndpointOutcome> {
    let source = self.find_source(&param.source)?;
    let feed = source
      .fetch_feed(Some(&self.client), param.base.as_ref())
      .await?;
    let mut context = FilterContext::new();
    if let Some(limit_filters) = param.limit_filters {
      context.set_limit_filters(limit_filters);
    }
    if let Some(base) = param.base {
      context.set_base(base);
    }

    let mut feed = self.filters.run(context, feed).await?;

    if let Some(limit) = param.limit_posts {
      let mut posts = feed.take_posts();
      posts.truncate(limit);
      feed.set_posts(posts);
    }

    let mut outcome = feed.into_outcome()?;
    if param.pretty_print {
      outcome.prettify();
    }
    Ok(outcome)
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

fn deserialize_bool_in_query<'de, D>(de: D) -> Result<bool, D::Error>
where
  D: serde::Deserializer<'de>,
{
  let s = String::deserialize(de)?;
  let b = s == "1" || s == "true";
  Ok(b)
}
