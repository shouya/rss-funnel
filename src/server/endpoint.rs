use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::response::IntoResponse;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use tower::Service;
use url::Url;

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
  source: Option<String>,
  content_type: Option<String>,
  filters: Vec<FilterConfig>,
}

#[derive(Clone)]
pub struct EndpointService {
  source: Option<String>,
  content_type: Option<String>,
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
    let fut = async {
      fut.await.or_else(|e| {
        Ok((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())
      })
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

    let client = reqwest::ClientBuilder::new()
      .user_agent(crate::util::USER_AGENT)
      .timeout(Duration::from_secs(10))
      .build()?;

    Ok(Self {
      source: config.source,
      content_type: config.content_type,
      filters: Arc::new(filters),
      client: Arc::new(client),
    })
  }

  async fn call_internal(self, req: Request) -> Result<Response> {
    let source = self.find_source(&req)?;
    let mut feed = self.fetch_feed(&source).await?;
    for filter in self.filters.iter() {
      filter.run(&mut feed).await?;
    }

    let resp = feed.into_resp()?;
    Ok(resp.into_response())
  }

  fn find_source(&self, req: &Request) -> Result<Url> {
    match self.source.as_ref() {
      Some(source) => Ok(Url::parse(&source)?),
      None => {
        // the uri from request only contains the path, so we need to
        // reconstruct the full url
        let url = Url::parse(&format!("http://placeholder{}", &req.uri()))?;
        let source = url
          .query_pairs()
          .find_map(|(k, v)| (k == "source").then(|| v))
          .ok_or(Error::Message(format!("missing source parameter")))?;
        Ok(Url::parse(&source)?)
      }
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
      .get("Content-Type")
      .and_then(|x| x.to_str().ok())
      // remove anything after ";"
      .and_then(|x| x.split(';').next())
      .map(|s| s.to_owned());
    let content_type = self
      .content_type
      .as_deref()
      .or(resp_content_type.as_deref());

    let content = resp.text().await?;

    let feed = match content_type {
      Some("text/html") => Feed::from_html_content(&content, &source)?,
      Some("application/xml")
      | Some("application/rss+xml")
      | Some("text/xml") => Feed::from_rss_content(&content)?,
      // todo: atom handling, etc.
      x => todo!("{:?}", x),
    };

    Ok(feed)
  }
}
