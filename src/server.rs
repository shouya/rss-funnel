pub(crate) mod endpoint;
mod inspector;

use std::{path::Path, sync::Arc, time::Duration};

use axum::{routing::get, Router};
use clap::Parser;
use http::StatusCode;
use tokio::sync::mpsc;
use tower_http::compression::CompressionLayer;
use tracing::{error, info};

use crate::{
  cli::FeedDefinition,
  util::{ConfigError, Result},
};
pub use endpoint::{EndpointConfig, EndpointOutcome, EndpointParam};

#[derive(Parser, Clone)]
pub struct ServerConfig {
  #[clap(long, short, default_value = "127.0.0.1:4080")]
  bind: Arc<str>,
  #[clap(
    long,
    action = clap::ArgAction::Set,
    num_args = 0..=1,
    require_equals = true,
    default_value = "true",
    default_missing_value = "true"
  )]
  inspector_ui: bool,
}

impl ServerConfig {
  pub async fn run(self, config_path: &Path) -> Result<()> {
    self.run_with_fs_watcher(config_path).await
  }

  #[allow(unused)]
  pub async fn run_without_fs_watcher(self, config_path: &Path) -> Result<()> {
    let config = FeedDefinition::load_from_file(config_path)?;
    self.serve(config).await
  }

  pub async fn run_with_fs_watcher(self, config_path: &Path) -> Result<()> {
    // watcher must not be dropped until the end of the function
    let (_watcher, mut config_update) = fs_watcher(config_path).await?;
    let Some(config) = config_update.recv().await else {
      return Err(
        ConfigError::Message("failed to load initial config".to_string())
          .into(),
      );
    };
    let mut task_handle = tokio::task::spawn(self.clone().serve(config));
    let mut config_update = debounce(Duration::from_millis(500), config_update);

    while let Some(new_config) = config_update.recv().await {
      info!("config updated, restarting server");
      task_handle.abort();
      task_handle = tokio::task::spawn(self.clone().serve(new_config));
    }

    Ok(())
  }

  pub async fn serve(self, feed_definition: FeedDefinition) -> Result<()> {
    info!("listening on {}", &self.bind);
    let listener = tokio::net::TcpListener::bind(&*self.bind).await?;

    let mut app = Router::new();

    for endpoint_config in feed_definition.clone().endpoints {
      info!("adding endpoint {}", &endpoint_config.path);
      let endpoint_route = endpoint_config.into_route().await?;
      app = app.merge(endpoint_route);
    }

    if self.inspector_ui {
      app = app.nest("/", inspector::router(feed_definition))
    } else {
      app = app.route("/", get(|| async { "rss-funnel is up and running!" }));
    }

    app = app
      .route("/health", get(|| async { "ok" }))
      .fallback(get(|| async {
        (StatusCode::NOT_FOUND, "Endpoint not found")
      }))
      .layer(CompressionLayer::new().gzip(true));

    info!("starting server");
    let server = axum::serve(listener, app);

    Ok(server.await?)
  }
}

async fn fs_watcher(
  config_path: &Path,
) -> Result<(notify::RecommendedWatcher, mpsc::Receiver<FeedDefinition>)> {
  use notify::{Event, RecursiveMode, Watcher};

  let (tx, rx) = mpsc::channel(1);
  let feed_definition = FeedDefinition::load_from_file(config_path).unwrap();

  tx.send(feed_definition)
    .await
    .expect("failed to send initial feed definition");

  let path = config_path.to_owned();
  let event_handler = move |event: Result<Event, notify::Error>| match event {
    Ok(event) if event.kind.is_modify() => {
      let feed_definition = FeedDefinition::load_from_file(&path).unwrap();
      tx.blocking_send(feed_definition).unwrap();
    }
    Ok(_) => {}
    Err(_) => {
      dbg!(&event);
      error!("file watcher error: {:?}", event);
    }
  };

  let mut watcher =
    notify::recommended_watcher(event_handler).map_err(|e| {
      ConfigError::Message(format!("failed to create file watcher: {:?}", e))
    })?;

  let path = config_path.to_owned();

  watcher
    .watch(&path, RecursiveMode::NonRecursive)
    .map_err(|e| {
      ConfigError::Message(format!("failed to watch file: {:?}", e))
    })?;

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
