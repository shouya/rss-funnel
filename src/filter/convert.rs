use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
  feed::{Feed, FeedFormat},
  util::{ConfigError, Result},
};

use super::{FeedFilter, FeedFilterConfig, FilterContext};

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
/// Convert feed to another format
pub struct ConvertToConfig {
  format: FeedFormat,
}

pub struct ConvertTo {
  format: FeedFormat,
}

#[async_trait::async_trait]
impl FeedFilterConfig for ConvertToConfig {
  type Filter = ConvertTo;

  async fn build(self) -> Result<Self::Filter, ConfigError> {
    Ok(ConvertTo {
      format: self.format,
    })
  }
}

#[async_trait::async_trait]
impl FeedFilter for ConvertTo {
  async fn run(&self, _ctx: &mut FilterContext, feed: Feed) -> Result<Feed> {
    Ok(feed.into_format(self.format))
  }
}
