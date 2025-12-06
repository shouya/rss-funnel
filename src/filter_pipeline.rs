use anyhow::Context;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info};

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
  pub filter_configs: Vec<FilterConfig>,
}

impl From<Vec<FilterConfig>> for FilterPipelineConfig {
  fn from(filter_configs: Vec<FilterConfig>) -> Self {
    Self { filter_configs }
  }
}

impl FilterPipelineConfig {
  pub fn is_empty(&self) -> bool {
    self.filter_configs.is_empty()
  }

  pub fn iter(&self) -> impl Iterator<Item = &FilterConfig> {
    self.filter_configs.iter()
  }
}

pub struct FilterPipeline {
  inner: RwLock<FilterPipelineInner>,
}

struct FilterPipelineInner {
  filters: Vec<CachedFilter>,
}

struct CachedFilter {
  position: usize,
  filter: BoxedFilter,
  config: FilterConfig,
  cache: FilterCache,
}

impl CachedFilter {
  async fn from_config(config: FilterConfig, position: usize) -> Result<Self> {
    let filter = config
      .clone()
      .build()
      .await
      .with_context(|| InFilterConfig(position, config.name().to_string()))?;
    let cache = FilterCache::new();

    Ok(Self {
      position,
      filter,
      config,
      cache,
    })
  }

  async fn run(&self, context: &mut FilterContext, feed: Feed) -> Result<Feed> {
    self
      .cache
      .run(feed, self.filter.cache_granularity(), |feed| {
        self.filter.run(context, feed)
      })
      .await
      .context(InFilter(self.position))
  }
}

impl From<Vec<CachedFilter>> for FilterPipelineInner {
  fn from(filters: Vec<CachedFilter>) -> Self {
    Self { filters }
  }
}

impl FilterPipeline {
  pub async fn from_config(config: FilterPipelineConfig) -> Result<Self> {
    let inner = FilterPipelineInner::from_config(config).await?;
    Ok(Self {
      inner: RwLock::new(inner),
    })
  }

  pub async fn run(
    &self,
    context: &mut FilterContext,
    feed: Feed,
  ) -> Result<Feed> {
    let inner = self.inner.read().await;
    inner.run(context, feed).await
  }

  pub async fn update(&self, config: FilterPipelineConfig) -> Result<()> {
    let mut inner = self.inner.write().await;

    for (i, filter_config) in config.filter_configs.into_iter().enumerate() {
      inner.replace_or_build(i, filter_config).await?;
    }

    Ok(())
  }
}

impl FilterPipelineInner {
  async fn from_config(config: FilterPipelineConfig) -> Result<Self> {
    let mut filters = Vec::new();

    for (i, filter_config) in config.filter_configs.into_iter().enumerate() {
      let filter = CachedFilter::from_config(filter_config, i).await?;
      filters.push(filter);
    }

    Ok(Self { filters })
  }

  async fn replace_or_build(
    &mut self,
    position: usize,
    config: FilterConfig,
  ) -> Result<()> {
    if self.filters.get(position).map(|x| &x.config) == Some(&config) {
      debug!("using cached filter: {}", config.name());
      return Ok(());
    }

    info!("rebuilding filter: {}", config.name());
    let filter = CachedFilter::from_config(config, position).await?;

    if self.filters.len() >= position {
      self.filters.truncate(position);
    }

    self.filters.push(filter);
    Ok(())
  }

  async fn run(
    &self,
    context: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    for filter in &self.filters {
      if !context.allows_filter(filter.position) {
        continue;
      }

      feed = filter.run(context, feed).await?;
    }

    Ok(feed)
  }
}
