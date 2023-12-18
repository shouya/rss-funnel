use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::body::Body;
use axum::response::IntoResponse;
use http_body_util::BodyExt;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use serde::{Deserialize, Serialize};
use tower::Service;

use crate::feed::Feed;
use crate::filter::{BoxedFilter, FeedFilter, FilterConfig};
use crate::util::{Error, Result};

type Request = http::Request<Body>;
type Response = http::Response<Body>;

#[derive(Serialize, Deserialize)]
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
}

#[derive(Serialize, Deserialize)]
pub struct EndpointServiceConfig {
  source: String,
  filters: Vec<FilterConfig>,
}

#[derive(Clone)]
pub struct EndpointService {
  source: String,
  filters: Arc<Vec<BoxedFilter>>,
  client: Arc<Client<HttpConnector, Body>>,
}

impl Service<Request> for EndpointService {
  type Response = Response;
  type Error = Infallible;
  type Future =
    Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn poll_ready(
    &mut self,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), Self::Error>> {
    std::task::Poll::Ready(Ok(()))
  }

  fn call(&mut self, req: Request) -> Self::Future {
    let this = self.clone();
    let fut = async { this.call_internal(req).await };
    let fut = async { fut.await.map_err(|_| unreachable!()) };
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

    let executor = hyper_util::rt::TokioExecutor::new();
    let client = Client::builder(executor).build_http();

    Ok(Self {
      source: config.source,
      filters: Arc::new(filters),
      client: Arc::new(client),
    })
  }

  async fn call_internal(self, _req: Request) -> Result<Response> {
    let mut feed = self.fetch_feed().await?;
    for filter in self.filters.iter() {
      filter.run(&mut feed).await?;
    }

    let resp = feed.into_resp()?;
    Ok(resp.into_response())
  }

  async fn fetch_feed(&self) -> Result<Feed> {
    let req = http::Request::builder()
      .uri(&self.source)
      .header("User-Agent", Self::user_agent())
      .header("Accept", "text/html,application/xml")
      .body(Body::empty())?;

    let resp: Response = self.client.request(req).await?.into_response();
    if resp.status() != http::StatusCode::OK {
      return Err(Error::UpstreamNon2xx(resp));
    }

    let (parts, body) = resp.into_parts();
    let content = body.collect().await?.to_bytes();

    let headers = &parts.headers;
    let feed = match headers.get("Content-Type").map(|x| x.as_ref()) {
      Some(b"text/html") => todo!(),
      Some(b"application/xml") => Feed::from_rss_content(&content)?,
      // todo: atom handling, etc.
      _ => todo!(),
    };

    Ok(feed)
  }

  fn user_agent() -> &'static str {
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"))
  }
}
