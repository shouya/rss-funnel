use std::time::Duration;

use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{error::Result, feed::Feed};

use super::{FeedFilter, FeedFilterConfig, FilterContext};

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(untagged)]
pub enum LimitConfig {
  Count(LimitByCount),
  Duration(LimitByDuration),
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
/// Only this many posts are kept.
pub struct LimitByCount(usize);

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
/// Only posts published within this duration are kept. (Examples: "1d", "3w")
pub struct LimitByDuration(
  #[serde(deserialize_with = "duration_str::deserialize_duration")]
  #[schemars(with = "String")]
  Duration,
);

pub struct Limit {
  config: LimitConfig,
}

#[async_trait::async_trait]
impl FeedFilterConfig for LimitConfig {
  type Filter = Limit;

  async fn build(self) -> Result<Self::Filter> {
    Ok(Limit { config: self })
  }
}

#[async_trait::async_trait]
impl FeedFilter for Limit {
  async fn run(
    &self,
    _ctx: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    match &self.config {
      LimitConfig::Count(LimitByCount(count)) => {
        let mut posts = feed.take_posts();
        posts.truncate(*count);
        feed.set_posts(posts);
        Ok(feed)
      }
      LimitConfig::Duration(LimitByDuration(duration)) => {
        let cutoff = Utc::now() - *duration;
        let mut posts = feed.take_posts();
        posts.retain(|post| post.pub_date().is_some_and(|t| t >= cutoff));
        feed.set_posts(posts);
        Ok(feed)
      }
    }
  }
}
