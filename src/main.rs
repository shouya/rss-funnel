mod cache;
mod cli;
mod client;
mod config;
mod error;
mod feed;
mod filter;
mod filter_pipeline;
mod html;
mod js;
mod otf_filter;
mod server;
mod source;
mod util;

#[cfg(test)]
mod test_utils;

use clap::Parser;

use crate::error::{ConfigError, Error, Result};

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt::init();

  #[cfg(unix)]
  {
    tokio::spawn(async {
      signal_handler().await.expect("Signal handler failed");
    });
  }

  let cli = cli::Cli::parse();
  cli.run().await
}

#[cfg(unix)]
async fn signal_handler() -> Result<()> {
  use tokio::signal::unix::{signal, SignalKind};
  use tracing::info;

  let mut sigint = signal(SignalKind::interrupt())?;
  let mut sigterm = signal(SignalKind::terminate())?;

  tokio::select! {
    _ = sigint.recv() => {
      info!("Received SIGINT, shutting down...");
    }
    _ = sigterm.recv() => {
      info!("Received SIGTERM, shutting down...");
    }
  };

  std::process::exit(0)
}
