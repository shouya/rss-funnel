use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;

use crate::{
  feed::Feed,
  filter::{BoxedFilter, FeedFilter, FilterConfig, FilterContext},
  filter_cache::FilterCache,
  ConfigError, Result,
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
  filter_cache: Option<FilterCache>,
}

impl FilterPipelineConfig {
  pub async fn build(self) -> Result<FilterPipeline, ConfigError> {
    let mut filters = vec![];
    let configs = self.filters.clone();
    for filter_config in self.filters {
      let filter = filter_config.build().await?;
      filters.push(filter);
    }

    let inner = Mutex::new(Inner {
      filters,
      configs,
      filter_cache: None,
    });
    Ok(FilterPipeline { inner })
  }
}

impl FilterPipeline {
  pub async fn run(&self, context: FilterContext, feed: Feed) -> Result<Feed> {
    self.inner.lock().await.run(context, feed).await
  }

  pub async fn update(
    &self,
    config: FilterPipelineConfig,
  ) -> Result<(), ConfigError> {
    let mut inner = self.inner.lock().await;
    let mut filters = vec![];
    let mut configs = vec![];

    for filter_config in config.filters {
      configs.push(filter_config.clone());
      match inner.take(&filter_config) {
        Some(filter) => filters.push(filter),
        None => {
          info!("building filter: {}", filter_config.name());
          let filter = filter_config.build().await?;
          filters.push(filter);
        }
      }
    }

    *inner = Inner {
      filters,
      configs,
      filter_cache: None,
    };
    Ok(())
  }
}

impl Inner {
  fn take(&mut self, config: &FilterConfig) -> Option<BoxedFilter> {
    let index = self.configs.iter().position(|c| c == config)?;
    self.configs.remove(index);
    Some(self.filters.remove(index))
  }

  async fn step(
    &self,
    filter: &BoxedFilter,
    context: &mut FilterContext,
    feed: Feed,
  ) -> Result<Feed> {
    if let Some(cache) = self.filter_cache.as_ref() {
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
    mut context: FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    for (i, filter) in self.filters.iter().enumerate() {
      if context.allows_filter(i) {
        feed = self.step(filter, &mut context, feed).await?;
      }
    }

    Ok(feed)
  }
}
