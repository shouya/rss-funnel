use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::body::Body;
use axum::response::IntoResponse;
use http::StatusCode;
use mime::Mime;
use serde::{Deserialize, Serialize};
use tower::Service;
use url::Url;

use crate::client::ClientConfig;
use crate::feed::Feed;
use crate::filter::{BoxedFilter, FeedFilter, FilterConfig};
use crate::util::{Error, Result};

type Request = http::Request<Body>;
type Response = http::Response<Body>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EndpointConfig {
  pub path: String,
  pub note: Option<String>,
  #[serde(flatten)]
  pub config: EndpointServiceConfig,
}

impl EndpointConfig {
  pub async fn into_route(self) -> Result<axum::Router> {
    let endpoint_service = EndpointService::from_config(self.config).await?;
    Ok(axum::Router::new().nest_service(&self.path, endpoint_service))
  }

  pub async fn into_service(self) -> Result<EndpointService> {
    EndpointService::from_config(self.config).await
  }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EndpointServiceConfig {
  source: Option<String>,
  content_type: Option<String>,
  filters: Vec<FilterConfig>,
  #[serde(default)]
  client: Option<ClientConfig>,
}

#[derive(Clone)]
pub struct EndpointService {
  source: Option<Url>,
  content_type: Option<String>,
  filters: Arc<Vec<BoxedFilter>>,
  client: Arc<reqwest::Client>,
}

#[derive(Clone, Default)]
pub struct EndpointParam {
  source: Option<Url>,
  /// Only process the initial N filter steps
  limit_filters: Option<usize>,
  /// Limit the number of items in the feed
  limit_posts: Option<usize>,
  pretty_print: bool,
}

impl EndpointParam {
  pub fn new(
    source: Option<Url>,
    limit_filters: Option<usize>,
    limit_posts: Option<usize>,
    pretty_print: bool,
  ) -> Self {
    Self {
      source,
      limit_filters,
      limit_posts,
      pretty_print,
    }
  }

  fn from_request(req: &Request) -> Self {
    Self {
      source: Self::parse_source(req),
      limit_filters: Self::parse_limit_filters(req),
      limit_posts: Self::parse_limit_posts(req),
      pretty_print: Self::parse_pretty_print(req),
    }
  }

  fn parse_source(req: &Request) -> Option<Url> {
    Self::get_query(req, "source").and_then(|x| Url::parse(&x).ok())
  }

  fn parse_limit_filters(req: &Request) -> Option<usize> {
    Self::get_query(req, "limit_filters").and_then(|x| x.parse::<usize>().ok())
  }

  fn parse_limit_posts(req: &Request) -> Option<usize> {
    Self::get_query(req, "limit_posts").and_then(|x| x.parse::<usize>().ok())
  }

  fn parse_pretty_print(req: &Request) -> bool {
    Self::get_query(req, "pp")
      .map(|x| x == "1" || x == "true")
      .unwrap_or(false)
  }

  fn get_query(req: &Request, name: &str) -> Option<String> {
    let url = Url::parse(&format!("http://placeholder{}", &req.uri())).ok()?;
    url
      .query_pairs()
      .find_map(|(k, v)| (k == name).then_some(v))
      .map(|x| x.to_string())
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

  fn call(&mut self, req: EndpointParam) -> Self::Future {
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

  fn call(&mut self, req: Request) -> Self::Future {
    let this = self.clone();
    let param = EndpointParam::from_request(&req);
    let fut = async { this.call_internal(param).await };
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
  pub async fn from_config(config: EndpointServiceConfig) -> Result<Self> {
    let mut filters = Vec::new();
    for filter_config in config.filters {
      let filter = filter_config.build().await?;
      filters.push(filter);
    }

    let client = config.client.unwrap_or_default().build()?;
    let source = match config.source {
      Some(source) => Some(Url::parse(&source)?),
      None => None,
    };

    Ok(Self {
      source,
      content_type: config.content_type,
      filters: Arc::new(filters),
      client: Arc::new(client),
    })
  }

  async fn call_internal(
    self,
    param: EndpointParam,
  ) -> Result<EndpointOutcome> {
    let source = self.find_source(&param.source)?;
    let mut feed = self.fetch_feed(&source).await?;
    let limited_filters = self
      .filters
      .iter()
      .take(param.limit_filters.unwrap_or(usize::MAX));
    for filter in limited_filters {
      filter.run(&mut feed).await?;
    }

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

  fn find_source(&self, param: &Option<Url>) -> Result<Url> {
    match self.source {
      // ignore the source from param if it's already specified in config
      Some(ref source) => Ok(source.clone()),
      None => param
        .as_ref()
        .ok_or(Error::Message("missing source".into()))
        .cloned(),
    }
  }

  async fn fetch_feed(&self, source: &Url) -> Result<Feed> {
    let resp = self
      .client
      .get(source.to_string())
      .header("Accept", "text/html,application/xml")
      .send()
      .await?
      .error_for_status()?;

    let resp_content_type = resp
      .headers()
      .get("content-type")
      .and_then(|x| x.to_str().ok())
      .and_then(|x| x.parse::<Mime>().ok())
      .map(|x| x.essence_str().to_owned());
    let content_type = self
      .content_type
      .as_deref()
      .or(resp_content_type.as_deref());

    let content = resp.text().await?;

    let feed = match content_type {
      Some("text/html") => Feed::from_html_content(&content, source)?,
      Some("application/xml")
      | Some("application/rss+xml")
      | Some("text/xml") => Feed::from_rss_content(&content)?,
      Some("application/atom+xml") => Feed::from_atom_content(&content)?,
      x => todo!("{:?}", x),
    };

    Ok(feed)
  }
}
