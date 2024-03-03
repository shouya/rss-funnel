use std::{collections::HashMap, ops::DerefMut, sync::Arc};

use axum::{
  extract::{Path, Request},
  response::{IntoResponse, Response},
  Extension,
};
use http::StatusCode;
use tokio::sync::RwLock;
use tower::Service;
use tracing::info;

use crate::{cli::RootConfig, util::ConfigError};

use super::{endpoint::EndpointService, EndpointConfig};

#[derive(Clone)]
pub struct FeedService {
  inner: Arc<RwLock<Inner>>,
}

struct Inner {
  config_error: Option<ConfigError>,
  root_config: Arc<RootConfig>,
  // maps path to service
  endpoints: HashMap<String, EndpointService>,
}

impl FeedService {
  pub async fn try_from(root_config: RootConfig) -> Result<Self, ConfigError> {
    let mut endpoints = HashMap::new();
    for endpoint_config in root_config.endpoints.clone() {
      let path = endpoint_config.path_sans_slash().to_owned();
      let endpoint_service = endpoint_config.build().await?;
      info!("loaded endpoint: {}", path);
      endpoints.insert(path, endpoint_service);
    }

    let inner = Inner {
      config_error: None,
      root_config: Arc::new(root_config),
      endpoints,
    };

    Ok(Self {
      inner: Arc::new(RwLock::new(inner)),
    })
  }

  pub async fn error<R>(&self, callback: impl FnOnce(&ConfigError) -> R) -> R {
    let inner = self.inner.read().await;
    match &inner.config_error {
      Some(e) => callback(e),
      None => panic!("no error"),
    }
  }

  pub async fn root_config(&self) -> Arc<RootConfig> {
    let inner = self.inner.read().await;
    inner.root_config.clone()
  }

  // Update the feed definition and reconfigure the services. Return true if
  // the reload was successful, false if there was an error.
  pub async fn reload(&self, path: &std::path::Path) -> bool {
    let mut inner = self.inner.write().await;
    inner.config_error = None;
    let feed_defn = match RootConfig::load_from_file(path) {
      Err(e) => {
        inner.config_error = Some(e);
        return false;
      }
      Ok(feed_defn) => feed_defn,
    };

    let mut endpoints = HashMap::new();
    for endpoint_config in feed_defn.endpoints.clone() {
      let path = endpoint_config.path_sans_slash().to_owned();
      let config = endpoint_config.clone();
      let endpoint = load_endpoint(&mut inner, &path, config).await;

      match endpoint {
        Ok(endpoint) => {
          endpoints.insert(path, endpoint);
        }
        Err(e) => {
          inner.config_error = Some(e);
          return false;
        }
      };
    }

    inner.root_config = Arc::new(feed_defn);
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
        format!("endpoint not defined: /{path}"),
      )
        .into_response(),
    }
  }
}

async fn load_endpoint(
  inner: &mut impl DerefMut<Target = Inner>,
  path: &str,
  config: EndpointConfig,
) -> Result<EndpointService, ConfigError> {
  match inner.endpoints.remove(path) {
    Some(endpoint) => {
      if !endpoint.config_changed(&config.config) {
        Ok(endpoint)
      } else {
        info!("endpoint updated, reloading: {}", path);
        endpoint.update(config.config).await
      }
    }
    None => config.build().await,
  }
}
