mod endpoint;

use axum::{routing::get, Router};
use clap::Parser;
pub use endpoint::EndpointConfig;
use http::StatusCode;
use tracing::info;

use crate::{cli::FeedDefinition, util::Result};

#[derive(Parser)]
pub struct ServerConfig {
  #[clap(long, short, default_value = "127.0.0.1:4080")]
  bind: String,
}

pub async fn serve(
  server_config: ServerConfig,
  feed_definition: FeedDefinition,
) -> Result<()> {
  info!("listening on {}", server_config.bind);
  let listener = tokio::net::TcpListener::bind(&server_config.bind).await?;

  let mut app = Router::new();
  for endpoint_config in feed_definition.endpoints {
    info!("adding endpoint {}", &endpoint_config.path);
    let endpoint_route = endpoint_config.into_route().await?;
    app = app.merge(endpoint_route);
  }

  app = app
    .route("/", get(|| async { "Up and running!" }))
    .route("/health", get(|| async { "ok" }))
    .fallback(get(|| async {
      (StatusCode::NOT_FOUND, "Endpoint not found")
    }));

  info!("starting server");
  Ok(axum::serve(listener, app).await?)
}
