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
    &mut self,
    mut feed: Feed,
    mut context: FilterContext,
    limit_filters: Option<usize>,
  ) -> Result<Feed> {
    let limit_filters = limit_filters.unwrap_or_else(|| self.num_filters());
    for filter in self.filters.iter().take(limit_filters) {
      filter.run(&mut context, &mut feed).await?;
    }
    Ok(feed)
  }

  pub fn num_filters(&self) -> usize {
    self.filters.len()
  }
}
