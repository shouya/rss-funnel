use anyhow::Context;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;

use crate::{
  error::{InFilter, InFilterConfig, Result},
  feed::Feed,
  filter::{BoxedFilter, FeedFilter, FilterConfig, FilterContext},
  filter_cache::FilterCache,
};

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
pub struct FilterPipelineConfig {
  pub filters: Vec<FilterConfig>,
}

impl From<Vec<FilterConfig>> for FilterPipelineConfig {
  fn from(filters: Vec<FilterConfig>) -> Self {
    Self { filters }
  }
}

pub struct FilterPipeline {
  inner: Mutex<Inner>,
}

struct Inner {
  filters: Vec<BoxedFilter>,
  configs: Vec<FilterConfig>,
  caches: Vec<FilterCache>,
}

impl FilterPipelineConfig {
  pub async fn build(self) -> Result<FilterPipeline> {
    let mut filters = vec![];
    let mut caches = vec![];
    let configs = self.filters.clone();

    for filter_config in self.filters {
      let filter = filter_config.build().await?;
      filters.push(filter);
      caches.push(FilterCache::new());
    }

    let inner = Mutex::new(Inner {
      filters,
      configs,
      caches,
    });
    Ok(FilterPipeline { inner })
  }
}

impl FilterPipeline {
  pub async fn run(
    &self,
    context: &mut FilterContext,
    feed: Feed,
  ) -> Result<Feed> {
    self.inner.lock().await.run(context, feed).await
  }

  pub async fn update(&self, config: FilterPipelineConfig) -> Result<()> {
    let mut inner = self.inner.lock().await;
    let mut filters = vec![];
    let mut configs = vec![];
    let mut caches = vec![];

    for (i, filter_config) in config.filters.into_iter().enumerate() {
      configs.push(filter_config.clone());

      if let Some((filter, cache)) = inner.take(&filter_config) {
        filters.push(filter);
        // preserve the cache if the filter is unchanged
        caches.push(cache);
      } else {
        let name = filter_config.name();
        info!("building filter: {}", filter_config.name());
        let filter = filter_config
          .build()
          .await
          .with_context(|| InFilterConfig(i, name))?;
        filters.push(filter);
        caches.push(FilterCache::new());
      }
    }

    *inner = Inner {
      filters,
      configs,
      caches,
    };
    Ok(())
  }
}

impl Inner {
  fn take(
    &mut self,
    config: &FilterConfig,
  ) -> Option<(BoxedFilter, FilterCache)> {
    let index = self.configs.iter().position(|c| c == config)?;
    let filter = self.filters.remove(index);
    let cache = self.caches.remove(index);
    self.configs.remove(index);
    Some((filter, cache))
  }

  async fn step(
    &self,
    index: usize,
    filter: &BoxedFilter,
    context: &mut FilterContext,
    feed: Feed,
  ) -> Result<Feed> {
    if let Some(cache) = self.caches.get(index) {
      let granularity = filter.cache_granularity();
      cache
        .run(feed, granularity, |feed| filter.run(context, feed))
        .await
    } else {
      filter.run(context, feed).await
    }
  }

  async fn run(
    &self,
    context: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    for (i, filter) in self.filters.iter().enumerate() {
      if context.allows_filter(i) {
        feed = self
          .step(i, filter, context, feed)
          .await
          .context(InFilter(i))?;
      }
    }

    Ok(feed)
  }
}
