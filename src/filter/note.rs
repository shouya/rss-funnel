use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{FeedFilter, FeedFilterConfig, FilterContext};
use crate::{feed::Feed, util::Result};

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct NoteFilterConfig {
  note: String,
}

#[async_trait::async_trait]
impl FeedFilterConfig for NoteFilterConfig {
  type Filter = NoteFilter;

  async fn build(self) -> Result<Self::Filter> {
    Ok(NoteFilter)
  }
}

pub struct NoteFilter;

#[async_trait::async_trait]
impl FeedFilter for NoteFilter {
  async fn run(&self, _ctx: &mut FilterContext, feed: Feed) -> Result<Feed> {
    Ok(feed)
  }
}
