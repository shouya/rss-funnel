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

use crate::util::Result;

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt::init();

  let cli = cli::Cli::parse();
  cli.run().await
}
