use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{
  feed::Feed,
  filter::{BoxedFilter, FeedFilter, FilterConfig, FilterContext},
  util::Result,
};

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
pub struct FilterPipelineConfig {
  filters: Vec<FilterConfig>,
}

pub struct FilterPipeline {
  inner: Mutex<Inner>,
}

struct Inner {
  filters: Vec<BoxedFilter>,
  configs: Vec<FilterConfig>,
}

impl FilterPipelineConfig {
  pub async fn build(self) -> Result<FilterPipeline> {
    let mut filters = Vec::new();
    let configs = self.filters.clone();
    for filter_config in self.filters {
      let filter = filter_config.build().await?;
      filters.push(filter);
    }

    let inner = Mutex::new(Inner { filters, configs });
    Ok(FilterPipeline { inner })
  }
}

impl FilterPipeline {
  pub async fn run(&self, context: FilterContext, feed: Feed) -> Result<Feed> {
    self.inner.lock().await.run(context, feed).await
  }

  pub fn num_filters(&self) -> usize {
    self.inner.blocking_lock().num_filters()
  }
}

impl Inner {
  async fn run(
    &self,
    mut context: FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    let limit_filters = context
      .limit_filters()
      .unwrap_or_else(|| self.num_filters());
    for filter in self.filters.iter().take(limit_filters) {
      feed = filter.run(&mut context, feed).await?;
    }
    Ok(feed)
  }

  fn num_filters(&self) -> usize {
    self.filters.len()
  }
}
