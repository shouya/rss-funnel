mod cli;
mod feed;
mod filter;
mod js;
mod server;
mod util;

use clap::Parser;

use crate::util::Result;

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt::init();

  let cli = cli::Cli::parse();
  cli.run().await
}
