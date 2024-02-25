use std::path::{Path, PathBuf};

use clap::Parser;
use schemars::JsonSchema;
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
  // boxed because of the clippy::large_enum_variant warning
  Test(Box<TestConfig>),
}

#[derive(Parser)]
struct TestConfig {
  /// The endpoint to test
  endpoint: String,
  /// The source URL to use for the endpoint
  #[clap(long, short)]
  source: Option<Url>,
  /// Limit the first N filter steps to run
  #[clap(long, short)]
  limit_filters: Option<usize>,
  /// Limit the number of items in the feed
  #[clap(long, short('n'))]
  limit_posts: Option<usize>,
  /// Whether to compact the XML output (opposite of pretty-print)
  #[clap(long, short)]
  compact_output: bool,
  /// Don't print XML output (Useful for checking console.log in JS filters)
  #[clap(long, short)]
  quiet: bool,
  /// The base URL of the feed, used for resolving relative urls
  #[clap(long)]
  base: Option<Url>,
}

impl TestConfig {
  fn to_endpoint_param(&self) -> server::EndpointParam {
    server::EndpointParam::new(
      self.source.as_ref().cloned(),
      self.limit_filters,
      self.limit_posts,
      !self.compact_output,
      self.base.clone(),
    )
  }
}

#[derive(JsonSchema, Serialize, Deserialize, Clone, Debug)]
pub struct FeedDefinition {
  pub endpoints: Vec<EndpointConfig>,
}

impl FeedDefinition {
  pub fn load_from_file(path: &Path) -> Result<Self> {
    let f = std::fs::File::open(path)?;
    let feed_definition =
      serde_yaml::from_reader(f).map_err(ConfigError::from)?;
    Ok(feed_definition)
  }

  fn get_endpoint(&self, endpoint: &str) -> Option<EndpointConfig> {
    self.endpoints.iter().find(|e| e.path == endpoint).cloned()
  }

  fn endpoints(&self) -> impl Iterator<Item = &EndpointConfig> {
    self.endpoints.iter()
  }
}

impl Cli {
  pub async fn run(self) -> Result<()> {
    match self.subcmd {
      SubCommand::Server(server_config) => {
        server_config.run(&self.config).await
      }
      SubCommand::Test(test_config) => {
        let feed_defn = FeedDefinition::load_from_file(&self.config)?;
        test_endpoint(feed_defn, &test_config).await;
        Ok(())
      }
    }
  }
}

async fn test_endpoint(feed_defn: FeedDefinition, test_config: &TestConfig) {
  let Some(endpoint_conf) = feed_defn.get_endpoint(&test_config.endpoint)
  else {
    let endpoints: Vec<_> =
      feed_defn.endpoints().map(|e| e.path.clone()).collect();
    eprintln!(
      "endpoint {} not found (available endpoints: {:?})",
      &test_config.endpoint, endpoints
    );
    return;
  };
  let mut endpoint_service = endpoint_conf
    .into_service()
    .await
    .expect("failed to build endpoint service");
  let endpoint_param = test_config.to_endpoint_param();
  let outcome = endpoint_service
    .call(endpoint_param)
    .await
    .expect("failed to call endpoint service");

  if !test_config.quiet {
    println!("{}", outcome.feed_xml());
  }
}
