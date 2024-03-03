pub(crate) mod endpoint;
mod feed_service;
#[cfg(feature = "inspector-ui")]
mod inspector;

use std::{path::Path, sync::Arc};

use axum::{routing::get, Extension, Router};
use clap::Parser;
use http::StatusCode;
use tokio::sync::mpsc;
use tower_http::compression::CompressionLayer;
use tracing::{error, info, warn};

use crate::{
  cli::RootConfig,
  util::{ConfigError, Result},
};
pub use endpoint::{EndpointConfig, EndpointOutcome, EndpointParam};

use self::feed_service::FeedService;

#[derive(Parser, Clone)]
pub struct ServerConfig {
  /// The address to bind to
  #[clap(long, short, default_value = "127.0.0.1:4080")]
  bind: Arc<str>,

  /// Whether to enable the inspector UI
  #[cfg(feature = "inspector-ui")]
  #[clap(
    long,
    action = clap::ArgAction::Set,
    num_args = 0..=1,
    require_equals = true,
    default_value = "true",
    default_missing_value = "true"
  )]
  inspector_ui: bool,

  /// Watch the config file for changes and restart the server
  #[clap(long, short)]
  watch: bool,
}

impl ServerConfig {
  pub async fn run(self, config_path: &Path) -> Result<()> {
    if self.watch {
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
    let (_watcher, mut config_update) = fs_watcher(config_path)?;

    // signal for reload on config update
    let feed_service_clone = feed_service.clone();
    let config_path_clone = config_path.to_owned();
    tokio::task::spawn(async move {
      while config_update.recv().await.is_some() {
        info!("config updated, reloading service");
        if !feed_service_clone.reload(&config_path_clone).await {
          feed_service_clone
            .error(|e| {
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

fn fs_watcher(
  config_path: &Path,
) -> Result<(notify::RecommendedWatcher, mpsc::Receiver<()>)> {
  use notify::{Event, RecursiveMode, Watcher};
  let (tx, rx) = mpsc::channel(1);

  let event_handler = move |event: Result<Event, notify::Error>| match event {
    Ok(event) if event.kind.is_modify() => {
      tx.blocking_send(()).unwrap();
    }
    Ok(_) => {}
    Err(_) => {
      error!("file watcher error: {:?}", event);
    }
  };

  let mut watcher =
    notify::recommended_watcher(event_handler).map_err(|e| {
      ConfigError::Message(format!("failed to create file watcher: {:?}", e))
    })?;

  watcher
    .watch(config_path, RecursiveMode::NonRecursive)
    .map_err(|e| {
      ConfigError::Message(format!("failed to watch file: {:?}", e))
    })?;

  // sometimes the editor may touch the file multiple times in quick
  // succession when saving, so we debounce the events
  let rx = debounce(std::time::Duration::from_millis(500), rx);
  Ok((watcher, rx))
}

fn debounce<T: Send + 'static>(
  duration: std::time::Duration,
  mut rx: mpsc::Receiver<T>,
) -> mpsc::Receiver<T> {
  let (debounced_tx, debounced_rx) = mpsc::channel(1);
  tokio::task::spawn(async move {
    let mut last = None;
    loop {
      tokio::select! {
        val = rx.recv() => {
          last = val;
        }
        _ = tokio::time::sleep(duration) => {
          if let Some(val) = last.take() {
            debounced_tx.send(val).await.unwrap();
          }
        }
      }
    }
  });
  debounced_rx
}
