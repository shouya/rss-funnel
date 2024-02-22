use serde::{Deserialize, Serialize};

use crate::{
  feed::Feed,
  filter::{BoxedFilter, FeedFilter, FilterConfig, FilterContext},
  util::Result,
};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(transparent)]
pub struct FilterPipelineConfig {
  filters: Vec<FilterConfig>,
}

#[derive(Clone)]
pub struct FilterPipeline {
  filters: Vec<BoxedFilter>,
}

impl FilterPipelineConfig {
  pub async fn build(self) -> Result<FilterPipeline> {
    let mut filters = Vec::new();
    for filter_config in self.filters {
      let filter = filter_config.build().await?;
      filters.push(filter);
    }
    Ok(FilterPipeline { filters })
  }
}

impl FilterPipeline {
  pub async fn run(
    &self,
    mut context: FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    let limit_filters =
      context.limit_filters.unwrap_or_else(|| self.num_filters());
    for filter in self.filters.iter().take(limit_filters) {
      feed = filter.run(&mut context, feed).await?;
    }
    Ok(feed)
  }

  pub fn num_filters(&self) -> usize {
    self.filters.len()
  }
}
