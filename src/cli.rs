use std::path::PathBuf;

use clap::Parser;
use serde::{Deserialize, Serialize};
use tower::Service;
use url::Url;

use crate::{
  server::{self, EndpointConfig, ServerConfig},
  util::{ConfigError, Result},
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
  #[clap(long, short)]
  source: Option<Url>,
  #[clap(long, short)]
  limit: Option<usize>,
  #[clap(long, short)]
  pretty_print: Option<bool>,
}

impl TestConfig {
  fn to_endpoint_param(&self) -> server::EndpointParam {
    server::EndpointParam::new(
      self.source.clone(),
      self.limit,
      self.pretty_print.unwrap_or(false),
    )
  }
}

#[derive(Serialize, Deserialize)]
pub struct FeedDefinition {
  pub endpoints: Vec<EndpointConfig>,
}

impl FeedDefinition {
  fn get_endpoint(&self, endpoint: &str) -> Option<EndpointConfig> {
    self.endpoints.iter().find(|e| e.path == endpoint).cloned()
  }

  fn endpoints(&self) -> impl Iterator<Item = &EndpointConfig> {
    self.endpoints.iter()
  }
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
    let feed_definition =
      serde_yaml::from_reader(f).map_err(ConfigError::from)?;
    Ok(feed_definition)
  }

  pub async fn run(self) -> Result<()> {
    let feed_defn = self.load_feed_definition()?;
    match self.subcmd {
      SubCommand::Server(server_config) => {
        server::serve(server_config, feed_defn).await
      }
      SubCommand::Test(test_config) => {
        test_endpoint(feed_defn, &test_config).await
      }
    }
  }
}

async fn test_endpoint(
  feed_defn: FeedDefinition,
  test_config: &TestConfig,
) -> Result<()> {
  let Some(endpoint_conf) = feed_defn.get_endpoint(&test_config.endpoint)
  else {
    let endpoints: Vec<_> =
      feed_defn.endpoints().map(|e| e.path.clone()).collect();
    return Err(crate::util::Error::Message(format!(
      "endpoint {} not found (available endpoints: {:?})",
      &test_config.endpoint, endpoints
    )));
  };
  let mut endpoint_service = endpoint_conf.into_service().await?;
  let endpoint_param = test_config.to_endpoint_param();
  let outcome = endpoint_service.call(endpoint_param).await?;
  println!("{}", outcome.feed_xml());

  Ok(())
}
