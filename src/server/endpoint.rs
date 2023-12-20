use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::body::Body;
use axum::response::IntoResponse;
use http_body_util::BodyExt;
use hyper_tls::HttpsConnector;
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

enum HttpClient {
  Http(Client<HttpConnector, Body>),
  Https(Client<HttpsConnector<HttpConnector>, Body>),
}

impl HttpClient {
  fn new(source: &str) -> Self {
    let executor = hyper_util::rt::TokioExecutor::new();

    if source.starts_with("http://") {
      let http_client = Client::builder(executor).build_http();
      Self::Http(http_client)
    } else if source.starts_with("https://") {
      let tls = hyper_tls::HttpsConnector::new();
      let https_client = Client::builder(executor).build(tls);
      Self::Https(https_client)
    } else {
      panic!(
        "invalid source: {}, must start with http:// or https://",
        source
      );
    }
  }

  async fn request(&self, request: Request) -> Result<Response> {
    match self {
      Self::Http(client) => {
        let resp: Response = client.request(request).await?.into_response();
        Ok(resp)
      }
      Self::Https(client) => {
        let resp: Response = client.request(request).await?.into_response();
        Ok(resp)
      }
    }
  }
}

#[derive(Clone)]
pub struct EndpointService {
  source: String,
  filters: Arc<Vec<BoxedFilter>>,
  client: Arc<HttpClient>,
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
    let fut = async { this.call_internal(req).await };
    let fut =
      async { fut.await.or_else(|e| Ok(e.to_string().into_response())) };
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

    let client = HttpClient::new(&config.source);

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
    let content_type = headers
      .get("Content-Type")
      .and_then(|x| x.to_str().ok())
      // remove anything after ";"
      .and_then(|x| x.split(';').next());

    let feed = match content_type {
      Some("text/html") => todo!(),
      Some("application/xml") => Feed::from_rss_content(&content)?,
      // todo: atom handling, etc.
      x => todo!("{:?}", x),
    };

    Ok(feed)
  }

  fn user_agent() -> &'static str {
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"))
  }
}
