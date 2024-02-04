use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::client::{Client, ClientConfig};
use crate::feed::Feed;
use crate::util::Result;

use super::{FeedFilter, FeedFilterConfig, FilterConfig, Filters};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum MergeConfig {
  Simple(MergeSimpleConfig),
  Full(MergeFullConfig),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct MergeSimpleConfig {
  source: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MergeFullConfig {
  source: String,
  #[serde(default)]
  client: ClientConfig,
  #[serde(default)]
  filters: Vec<FilterConfig>,
}

impl From<MergeSimpleConfig> for MergeFullConfig {
  fn from(config: MergeSimpleConfig) -> Self {
    Self {
      source: config.source,
      client: ClientConfig::default(),
      filters: Default::default(),
    }
  }
}

impl From<MergeConfig> for MergeFullConfig {
  fn from(config: MergeConfig) -> Self {
    match config {
      MergeConfig::Simple(config) => config.into(),
      MergeConfig::Full(config) => config,
    }
  }
}

#[async_trait::async_trait]
impl FeedFilterConfig for MergeConfig {
  type Filter = Merge;

  async fn build(self) -> Result<Self::Filter> {
    let MergeFullConfig {
      client,
      filters,
      source,
    } = self.into();
    let client = client.build(Duration::from_secs(15 * 60))?;
    let filters = Filters::from_config(filters).await?;
    let source = Url::parse(&source)?;

    Ok(Merge {
      client,
      source,
      filters,
    })
  }
}

pub struct Merge {
  client: Client,
  source: Url,
  filters: Filters,
}

#[async_trait::async_trait]
impl FeedFilter for Merge {
  async fn run(&self, feed: &mut Feed) -> Result<()> {
    let mut new_feed = self.client.fetch_feed(&self.source).await?;
    self.filters.process(&mut new_feed).await?;
    feed.merge(new_feed)?;
    Ok(())
  }
}
