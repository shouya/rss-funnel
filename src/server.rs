mod auth;
mod endpoint;
mod feed_service;
#[cfg(feature = "inspector-ui")]
mod inspector;
mod watcher;

use std::{path::Path, sync::Arc};

use axum::{routing::get, Extension, Router};
use clap::Parser;
use http::StatusCode;
use tower_http::compression::CompressionLayer;
use tracing::{info, warn};

use crate::{cli::RootConfig, util::Result};
pub use endpoint::{EndpointConfig, EndpointParam};

use self::{feed_service::FeedService, watcher::Watcher};

#[derive(Parser, Clone)]
pub struct ServerConfig {
  /// The address to bind to
  #[clap(
    long,
    short,
    default_value = "127.0.0.1:4080",
    env = "RSS_FUNNEL_BIND"
  )]
  bind: Arc<str>,

  /// Whether to enable the inspector UI
  #[cfg(feature = "inspector-ui")]
  #[clap(
    long,
    action = clap::ArgAction::Set,
    num_args = 0..=1,
    require_equals = true,
    default_value = "true",
    default_missing_value = "true",
    env = "RSS_FUNNEL_INSPECTOR_UI"
  )]
  inspector_ui: bool,

  /// Watch the config file for changes and restart the server
  #[clap(long, short, env = "RSS_FUNNEL_WATCH")]
  watch: bool,
}

impl ServerConfig {
  pub async fn run(self, config_path: &Path) -> Result<()> {
    if self.watch {
      info!("watching config file for changes");
      self.run_with_fs_watcher(config_path).await
    } else {
      self.run_without_fs_watcher(config_path).await
    }
  }

  #[allow(unused)]
  pub async fn run_without_fs_watcher(self, config_path: &Path) -> Result<()> {
    let config = RootConfig::load_from_file(config_path)?;
    let feed_service = FeedService::try_from(config).await?;
    self.serve(feed_service).await
  }

  pub async fn run_with_fs_watcher(self, config_path: &Path) -> Result<()> {
    let config = RootConfig::load_from_file(config_path)?;
    let feed_service = FeedService::try_from(config).await?;

    // watcher must not be dropped until the end of the function
    let mut watcher = Watcher::new(config_path)?;
    watcher.setup()?;
    let mut change_alert = watcher.take_change_alert().expect(" failed");

    // signal for reload on config update
    let feed_service_clone = feed_service.clone();
    let config_path_clone = config_path.to_owned();
    tokio::task::spawn(async move {
      while change_alert.recv().await.is_some() {
        info!("config updated, reloading service");
        if !feed_service_clone.reload(&config_path_clone).await {
          feed_service_clone
            .with_error(|e| {
              warn!("failed to reload config: {}", e);
            })
            .await;
        }
      }
    });

    self.serve(feed_service).await
  }

  pub async fn serve(self, feed_service: FeedService) -> Result<()> {
    info!("listening on {}", &self.bind);
    let listener = tokio::net::TcpListener::bind(&*self.bind).await?;

    let mut app = Router::new();

    #[cfg(feature = "inspector-ui")]
    if self.inspector_ui {
      app = app.nest("/", inspector::router())
    } else {
      app = app.route("/", get(|| async { "rss-funnel is up and running!" }));
    }

    if !cfg!(feature = "inspector-ui") {
      app = app.route("/", get(|| async { "rss-funnel is up and running!" }));
    }

    app = app
      .route("/health", get(|| async { "ok" }))
      .route("/:endpoint", get(FeedService::handler))
      .layer(Extension(feed_service))
      .fallback(get(|| async {
        (StatusCode::NOT_FOUND, "Endpoint not found")
      }))
      .layer(CompressionLayer::new().gzip(true));

    info!("starting server");
    let server = axum::serve(listener, app);

    Ok(server.await?)
  }
}
