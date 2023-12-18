mod config;
mod js;

use serde::de::DeserializeOwned;

use crate::{feed::Feed, util::Result};

pub use config::FilterConfig;

#[async_trait::async_trait]
pub trait FeedFilter {
  async fn run(&mut self, feed: &mut Feed) -> Result<()>;
}

#[async_trait::async_trait]
pub trait FeedFilterConfig: DeserializeOwned {
  type Filter: FeedFilter;

  async fn build(&self) -> Result<Self::Filter>;
}
