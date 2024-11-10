mod auth;
mod endpoint;
mod feed_service;
pub mod image_proxy;
#[cfg(feature = "inspector-ui")]
mod inspector;
mod watcher;
mod web;

use std::{path::Path, sync::Arc};

use axum::{
  response::{IntoResponse, Redirect},
  routing::get,
  Extension, Router,
};
use clap::Parser;
use http::StatusCode;
use tower_http::compression::CompressionLayer;
use tracing::{info, warn};

use crate::{
  cli::RootConfig,
  util::{self, relative_path},
  Result,
};

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
  pub async fn run(self, config_path: Option<&Path>) -> Result<()> {
    if let Some(config_path) = config_path {
      info!("loading config from {:?}", config_path);
      self.run_with_config(config_path).await
    } else {
      info!("running without config file");
      self.run_without_config().await
    }
  }

  pub async fn run_without_config(self) -> Result<()> {
    let feed_service = FeedService::new_otf().await?;
    let rel_path = relative_path("otf");
    info!(
      "No config detected. Serving automatic on-the-fly endpoint on {rel_path}"
    );
    self.serve(feed_service).await
  }

  pub async fn run_with_config(self, config_path: &Path) -> Result<()> {
    if self.watch {
      info!("watching config file for changes");
      self.run_with_fs_watcher(config_path).await
    } else {
      self.run_without_fs_watcher(config_path).await
    }
  }

  pub async fn run_without_fs_watcher(self, config_path: &Path) -> Result<()> {
    let feed_service = FeedService::new(config_path).await?;
    self.serve(feed_service).await
  }

  pub async fn run_with_fs_watcher(self, config_path: &Path) -> Result<()> {
    let feed_service = FeedService::new(config_path).await?;

    // watcher must not be dropped until the end of the function
    let mut watcher = Watcher::new(config_path)?;
    let mut change_alert =
      watcher.take_change_alert().expect("change alert taken");

    tokio::task::spawn(watcher.run());

    // signal for reload on config update
    let feed_service_clone = feed_service.clone();
    tokio::task::spawn(async move {
      while change_alert.recv().await.is_some() {
        info!("config updated, reloading service");
        if !feed_service_clone.reload().await {
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

  pub fn router(&self, feed_service: FeedService) -> Router {
    let mut routes = Router::new();

    #[cfg(feature = "inspector-ui")]
    if self.inspector_ui {
      routes = routes
        .nest("/", inspector::router())
        .nest("/_/", web::router())
        .route("/", get(redirect_to_home));
    } else {
      routes =
        routes.route("/", get(|| async { "rss-funnel is up and running!" }));
    }

    if !cfg!(feature = "inspector-ui") {
      routes =
        routes.route("/", get(|| async { "rss-funnel is up and running!" }));
    }

    let feed_service_router = Router::new()
      .route("/:endpoint", get(FeedService::handler))
      .fallback(get(|| async {
        (StatusCode::NOT_FOUND, "Endpoint not found")
      }))
      .layer(CompressionLayer::new().gzip(true));

    routes = routes
      // deprecated, will be removed on 0.2
      .route("/health", get(|| async { "ok" }))
      .route("/_health", get(|| async { "ok" }))
      .merge(image_proxy::router())
      .merge(feed_service_router)
      .layer(Extension(feed_service));

    routes
  }

  pub async fn serve(self, feed_service: FeedService) -> Result<()> {
    info!("listening on {}", &self.bind);
    let listener = tokio::net::TcpListener::bind(&*self.bind).await?;

    let mut app = Router::new();

    let prefix = util::relative_path("");
    if prefix == "/" {
      app = self.router(feed_service);
    } else {
      info!("Path prefix set to {prefix}");
      app = app.nest(&prefix, self.router(feed_service))
    };

    info!("starting server");
    let server = axum::serve(listener, app);

    Ok(server.await?)
  }
}

async fn redirect_to_home() -> impl IntoResponse {
  let home_path = relative_path("_/");
  Redirect::temporary(&home_path)
}
