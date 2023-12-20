mod config;
mod full_text;
mod js;

use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::{feed::Feed, util::Result};

pub use config::FilterConfig;

#[async_trait::async_trait]
pub trait FeedFilter {
  async fn run(&self, feed: &mut Feed) -> Result<()>;
}

#[async_trait::async_trait]
pub trait FeedFilterConfig: DeserializeOwned {
  type Filter: FeedFilter;

  async fn build(&self) -> Result<Self::Filter>;
}

#[derive(Clone)]
pub struct BoxedFilter(Arc<dyn FeedFilter + Send + Sync>);

#[async_trait::async_trait]
impl FeedFilter for BoxedFilter {
  async fn run(&self, feed: &mut Feed) -> Result<()> {
    self.0.run(feed).await
  }
}

impl BoxedFilter {
  fn from<T>(filter: T) -> Self
  where
    T: FeedFilter + Send + Sync + 'static,
  {
    Self(Arc::new(filter))
  }
}
