mod cache;
mod cli;
mod client;
mod config;
mod feed;
mod filter;
mod filter_pipeline;
mod html;
mod js;
mod otf_filter;
mod server;
mod source;
#[cfg(test)]
mod test_utils;
mod util;

use clap::Parser;
use tracing::info;

use crate::util::Result;

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt::init();

  tokio::spawn(async {
    signal_handler().await.expect("Signal handler failed");
  });

  let cli = cli::Cli::parse();
  cli.run().await
}

async fn signal_handler() -> Result<()> {
  use tokio::signal::unix::{signal, SignalKind};

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
