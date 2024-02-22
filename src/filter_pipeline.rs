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
  pub async fn process(
    &self,
    ctx: &mut FilterContext,
    feed: &mut Feed,
  ) -> Result<()> {
    self.process_partial(feed, ctx, self.filters.len()).await
  }

  pub async fn process_partial(
    &self,
    feed: &mut Feed,
    ctx: &mut FilterContext,
    limit_filters: usize,
  ) -> Result<()> {
    for filter in self.filters.iter().take(limit_filters) {
      filter.run(ctx, feed).await?;
    }
    Ok(())
  }
}
