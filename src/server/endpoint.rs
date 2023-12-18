use std::convert::Infallible;

use axum::body::Body;
use axum::response::IntoResponse;
use http::Request;
use serde::{Deserialize, Serialize};
use tower::Service;

use crate::filter::{FeedFilter, FilterConfig};
use crate::util::Result;

#[derive(Serialize, Deserialize)]
pub struct EndpointConfig {
  pub path: String,
  pub note: Option<String>,
  #[serde(flatten)]
  pub config: EndpointServiceConfig,
}

impl EndpointConfig {
  pub fn into_route(self) -> axum::Router {
    todo!()
  }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
  Html,
  Rss,
}

#[derive(Serialize, Deserialize)]
pub struct EndpointServiceConfig {
  source: String,
  filters: Vec<FilterConfig>,
}

pub struct EndpointService {
  source: String,
  source_type: SourceType,
  filters: Vec<Box<dyn FeedFilter>>,
}

impl EndpointService {
  pub async fn from_config(config: EndpointServiceConfig) -> Result<Self> {
    let mut filters = Vec::new();
    for filter_config in config.filters {
      let filter = filter_config.build().await?;
      filters.push(filter);
    }
    Ok(Self {
      source: config.source,
      source_type: SourceType::Html,
      filters,
    })
  }

  pub fn into_service<T>(self) -> T
  where
    T: Service<Request<Body>, Error = Infallible> + Clone + Send + 'static,
    T::Response: IntoResponse,
    T::Future: Send + 'static,
  {
    todo!()
  }
}
