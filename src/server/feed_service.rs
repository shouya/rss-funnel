use std::{collections::HashMap, ops::DerefMut, path::PathBuf, sync::Arc};

use axum::{
  Extension,
  extract::{Path, Request},
  response::{IntoResponse, Response},
};
use http::StatusCode;
use rand::{TryRngCore as _, rngs::OsRng};
use tokio::sync::RwLock;
use tower::Service;
use tracing::info;

use crate::cli::RootConfig;
use crate::error::{ConfigError, Result};

use super::{EndpointConfig, endpoint::EndpointService};

// can be cheaply cloned.
#[derive(Clone)]
pub struct FeedService {
  inner: Arc<RwLock<Inner>>,
}

struct Inner {
  config_path: Option<PathBuf>,
  config_error: Option<ConfigError>,
  root_config: Arc<RootConfig>,
  session_id: Option<String>,
  // maps path to service
  endpoints: HashMap<String, EndpointService>,
}

impl FeedService {
  pub async fn new_otf() -> Result<Self> {
    let config = RootConfig::on_the_fly("/otf");
    Self::build(config, None).await
  }

  pub async fn new(path: &std::path::Path) -> Result<Self> {
    let config = RootConfig::load_from_file(path)?;
    Self::build(config, Some(path)).await
  }
}

impl FeedService {
  async fn build(
    root_config: RootConfig,
    config_path: Option<&std::path::Path>,
  ) -> Result<Self> {
    let mut endpoints = HashMap::new();
    for endpoint_config in root_config.endpoints.clone() {
      let path = endpoint_config.path_sans_slash().to_owned();
      let endpoint_service = endpoint_config.build().await?;
      info!("loaded endpoint: /{}", path);
      if endpoints.contains_key(&path) {
        anyhow::bail!("duplicate endpoint: {path}");
      }

      endpoints.insert(path, endpoint_service);
    }

    let inner = Inner {
      config_path: config_path.map(PathBuf::from),
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
    OsRng
      .try_fill_bytes(&mut buffer)
      .expect("Failed to generate session id.");
    let mut session_id = String::with_capacity(32 * 2);
    for byte in &buffer {
      session_id.push_str(&format!("{byte:02x}"));
    }

    let mut inner = self.inner.write().await;
    inner.session_id = Some(session_id.clone());
    Some(session_id)
  }

  // Update the feed definition and reconfigure the services. Return true if
  // the reload was successful, false if there was an error.
  pub async fn reload(&self) -> bool {
    let Some(path) = self.inner.read().await.config_path.clone() else {
      // no path specified, no reload needed
      return true;
    };

    let mut inner = self.inner.write().await;
    inner.config_error = None;
    let feed_defn = match RootConfig::load_from_file(&path) {
      Err(e) => {
        inner.config_error = Some(ConfigError(e));
        return false;
      }
      Ok(feed_defn) => feed_defn,
    };

    let mut endpoints = HashMap::new();
    for endpoint_config in feed_defn.endpoints.clone() {
      let path = endpoint_config.path_sans_slash().to_owned();
      if endpoints.contains_key(&path) {
        inner.config_error =
          Some(ConfigError(anyhow::anyhow!("duplicate endpoint: {path}")));
        return false;
      }

      let config = endpoint_config.clone();
      let endpoint = load_endpoint(&mut inner, &path, config).await;

      match endpoint {
        Ok(endpoint) => {
          endpoints.insert(path, endpoint);
        }
        Err(e) => {
          inner.config_error = Some(ConfigError(e));
          return false;
        }
      }
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
) -> Result<EndpointService> {
  match inner.endpoints.remove(path) {
    Some(endpoint) => {
      if endpoint.config_changed(&config.config) {
        info!("endpoint updated, reloading: {}", path);
        endpoint.update(config.config).await
      } else {
        Ok(endpoint)
      }
    }
    None => config.build().await,
  }
}
