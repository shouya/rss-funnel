use std::{collections::HashMap, sync::Arc};

use axum::{
  extract::{Path, Request},
  response::{IntoResponse, Response},
  Extension,
};
use http::StatusCode;
use tokio::sync::RwLock;
use tower::Service;

use crate::{cli::FeedDefinition, util::ConfigError};

use super::endpoint::EndpointService;

#[derive(Clone)]
pub struct FeedService {
  inner: Arc<RwLock<Inner>>,
}

struct Inner {
  config_error: Option<ConfigError>,
  feed_definition: Arc<FeedDefinition>,
  // maps path to service
  endpoints: HashMap<String, EndpointService>,
}

impl FeedService {
  pub async fn try_from(
    feed_definition: FeedDefinition,
  ) -> Result<Self, ConfigError> {
    let mut endpoints = HashMap::new();
    for endpoint_config in feed_definition.endpoints.clone() {
      let path = endpoint_config.path.clone();
      let endpoint_service = endpoint_config.build().await?;
      endpoints.insert(path, endpoint_service);
    }

    let inner = Inner {
      config_error: None,
      feed_definition: Arc::new(feed_definition),
      endpoints,
    };

    Ok(Self {
      inner: Arc::new(RwLock::new(inner)),
    })
  }

  pub async fn feed_definition(&self) -> Arc<FeedDefinition> {
    let inner = self.inner.read().await;
    inner.feed_definition.clone()
  }
  // Update the feed definition and reconfigure the services. Return true if
  // the reload was successful, false if there was an error.
  pub async fn reload(&self, path: &std::path::Path) -> bool {
    let mut inner = self.inner.write().await;
    inner.config_error = None;
    let feed_defn = match FeedDefinition::load_from_file(path) {
      Err(e) => {
        inner.config_error = Some(e);
        return false;
      }
      Ok(feed_defn) => feed_defn,
    };

    let mut endpoints = HashMap::new();
    for endpoint_config in feed_defn.endpoints.clone() {
      let path = endpoint_config.path.clone();
      // TODO: instead of recreating all endpoints, update existing ones.
      match endpoint_config.build().await {
        Err(e) => {
          inner.config_error = Some(e);
          return false;
        }
        Ok(endpoint_service) => {
          endpoints.insert(path.clone(), endpoint_service);
        }
      };
    }

    inner.feed_definition = Arc::new(feed_defn);
    inner.endpoints = endpoints;
    true
  }

  pub async fn handler(
    Path(path): Path<String>,
    Extension(service): Extension<FeedService>,
    request: Request,
  ) -> Response {
    let inner = service.inner.read().await;
    match inner.endpoints.get(&path) {
      Some(endpoint) => {
        let mut endpoint = endpoint.clone();
        endpoint
          .call(request)
          .await
          .expect("infallible endpoint call failed")
      }
      _ => (
        StatusCode::NOT_FOUND,
        format!("endpoint not defined: {path}"),
      )
        .into_response(),
    }
  }
}
