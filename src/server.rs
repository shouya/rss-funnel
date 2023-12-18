mod endpoint;

use axum::Router;
use clap::Parser;
pub use endpoint::EndpointConfig;

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
  let mut app = Router::new();

  for endpoint_config in feed_definition.endpoints {
    let endpoint_route = endpoint_config.into_route().await?;
    app = app.merge(endpoint_route);
  }

  let listener = tokio::net::TcpListener::bind(&server_config.bind).await?;

  Ok(axum::serve(listener, app).await?)
}
