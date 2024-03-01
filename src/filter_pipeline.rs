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
  configs: Mutex<Vec<FilterConfig>>,
  filters: Mutex<Vec<BoxedFilter>>,
}

impl FilterPipelineConfig {
  pub async fn build(self) -> Result<FilterPipeline> {
    let mut filters = Vec::new();
    let configs = self.filters.clone();
    for filter_config in self.filters {
      let filter = filter_config.build().await?;
      filters.push(filter);
    }

    let filters = Mutex::new(filters);
    let configs = Mutex::new(configs);
    Ok(FilterPipeline { filters, configs })
  }
}

impl FilterPipeline {
  pub async fn run(
    &self,
    mut context: FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    let limit_filters = context
      .limit_filters()
      .unwrap_or_else(|| self.num_filters());
    let filters = self.filters.lock().await;
    for filter in filters.iter().take(limit_filters) {
      feed = filter.run(&mut context, feed).await?;
    }
    Ok(feed)
  }

  pub fn num_filters(&self) -> usize {
    self.filters.blocking_lock().len()
  }
}
