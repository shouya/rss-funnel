use std::{collections::HashMap, ops::DerefMut, sync::Arc};

use axum::{
  extract::{Path, Request},
  response::{IntoResponse, Response},
  Extension,
};
use http::StatusCode;
use rand::{rngs::OsRng, RngCore};
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
  session_id: Option<String>,
  // maps path to service
  endpoints: HashMap<String, EndpointService>,
}

impl FeedService {
  pub async fn try_from(root_config: RootConfig) -> Result<Self, ConfigError> {
    let mut endpoints = HashMap::new();
    for endpoint_config in root_config.endpoints.clone() {
      let path = endpoint_config.path_sans_slash().to_owned();
      let endpoint_service = endpoint_config.build().await?;
      info!("loaded endpoint: /{}", path);
      endpoints.insert(path, endpoint_service);
    }

    let inner = Inner {
      config_error: None,
      session_id: None,
      root_config: Arc::new(root_config),
      endpoints,
    };

    Ok(Self {
      inner: Arc::new(RwLock::new(inner)),
    })
  }

  pub async fn with_error<R>(
    &self,
    f: impl FnOnce(&ConfigError) -> R,
  ) -> Option<R> {
    let inner = self.inner.read().await;
    inner.config_error.as_ref().map(f)
  }

  pub async fn root_config(&self) -> Arc<RootConfig> {
    let inner = self.inner.read().await;
    inner.root_config.clone()
  }

  pub async fn requires_auth(&self) -> bool {
    let inner = self.inner.read().await;
    inner.root_config.auth.is_some()
  }

  pub async fn validate_session_id(&self, session_id: &str) -> bool {
    let inner = self.inner.read().await;
    inner.session_id.as_deref() == Some(session_id)
  }

  pub async fn login(&self, username: &str, password: &str) -> Option<String> {
    let inner = self.inner.read().await;
    let auth = inner.root_config.auth.as_ref()?;
    if !(auth.username == username && auth.password == password) {
      return None;
    }
    drop(inner);

    let mut buffer = [0u8; 32];
    OsRng.fill_bytes(&mut buffer);
    let session_id = format!("{:x?}", buffer);

    let mut inner = self.inner.write().await;
    inner.session_id = Some(session_id.clone());
    Some(session_id)
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

  pub async fn get_endpoint(&self, path: &str) -> Option<EndpointService> {
    let inner = self.inner.read().await;
    inner.endpoints.get(path).cloned()
  }

  pub async fn handler(
    Path(path): Path<String>,
    Extension(service): Extension<FeedService>,
    request: Request,
  ) -> Response {
    match service.get_endpoint(&path).await {
      Some(mut endpoint) => endpoint
        .call(request)
        .await
        .expect("infallible endpoint call failed"),
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
