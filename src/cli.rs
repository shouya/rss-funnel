use std::path::PathBuf;

use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::{
  server::{self, EndpointConfig, ServerConfig},
  util::Result,
};

#[derive(Parser)]
pub struct Cli {
  #[clap(subcommand)]
  subcmd: SubCommand,

  #[clap(long, short)]
  config: PathBuf,
}

#[derive(Parser)]
enum SubCommand {
  Server(ServerConfig),
  Test(TestConfig),
}

#[derive(Parser)]
struct TestConfig {
  endpoint: String,
}

#[derive(Serialize, Deserialize)]
pub struct FeedDefinition {
  pub endpoints: Vec<EndpointConfig>,
}

#[derive(Serialize, Deserialize)]
struct FollowLinkFilterConfig {
  /// Selector for the link elements from an html page
  link_selector: String,
  /// the attribute to look in the element for the link, defaults to "href"
  #[serde(default = "default_href_attr")]
  href_attr: String,
}

fn default_href_attr() -> String {
  "href".to_string()
}

impl Cli {
  fn load_feed_definition(&self) -> Result<FeedDefinition> {
    let f = std::fs::File::open(&self.config)?;
    let feed_definition = serde_yaml::from_reader(f)?;
    Ok(feed_definition)
  }

  pub async fn run(self) -> Result<()> {
    let feed_defn = self.load_feed_definition()?;
    match self.subcmd {
      SubCommand::Server(server_config) => {
        server::serve(server_config, feed_defn).await
      }
      SubCommand::Test(_test_config) => {
        todo!()
      }
    }
  }
}
