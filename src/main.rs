mod cli;
mod feed;
mod filter;
mod server;
mod util;

use clap::Parser;

use crate::util::Result;

#[tokio::main]
async fn main() -> Result<()> {
  let cli = cli::Cli::parse();
  cli.run().await
}
