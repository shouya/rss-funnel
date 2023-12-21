use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use tower::Service;

use crate::feed::Feed;
use crate::filter::{BoxedFilter, FeedFilter, FilterConfig};
use crate::util::Result;

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
  client: Arc<reqwest::Client>,
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

    let client = reqwest::ClientBuilder::new()
      .user_agent(crate::util::USER_AGENT)
      .timeout(Duration::from_secs(10))
      .build()?;

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
    let resp = self
      .client
      .get(&self.source)
      .header("Accept", "text/html,application/xml")
      .send()
      .await?
      .error_for_status()?;

    let content_type = resp
      .headers()
      .get("Content-Type")
      .and_then(|x| x.to_str().ok())
      // remove anything after ";"
      .and_then(|x| x.split(';').next())
      .map(|s| s.to_owned());

    let content = resp.text().await?;

    let feed = match content_type.as_deref() {
      Some("text/html") => todo!(),
      Some("application/xml") => Feed::from_rss_content(&content)?,
      // todo: atom handling, etc.
      x => todo!("{:?}", x),
    };

    Ok(feed)
  }
}
