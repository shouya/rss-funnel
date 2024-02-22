mod full_text;
mod highlight;
mod html;
mod js;
mod merge;
mod sanitize;
mod select;
mod simplify_html;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{feed::Feed, util::Result};

pub struct FilterContext {}

impl FilterContext {
  pub fn new() -> Self {
    Self {}
  }
}

#[async_trait::async_trait]
pub trait FeedFilter {
  async fn run(&self, ctx: &FilterContext, feed: &mut Feed) -> Result<()>;
}

#[async_trait::async_trait]
pub trait FeedFilterConfig {
  type Filter: FeedFilter;

  async fn build(self) -> Result<Self::Filter>;
}

#[derive(Clone)]
pub struct BoxedFilter(Arc<dyn FeedFilter + Send + Sync>);

#[async_trait::async_trait]
impl FeedFilter for BoxedFilter {
  async fn run(&self, ctx: &FilterContext, feed: &mut Feed) -> Result<()> {
    self.0.run(ctx, feed).await
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

pub struct Filters {
  filters: Vec<BoxedFilter>,
}

impl Filters {
  pub async fn from_config(filter_configs: Vec<FilterConfig>) -> Result<Self> {
    let mut filters = Vec::new();
    for filter_config in filter_configs {
      let filter = filter_config.build().await?;
      filters.push(filter);
    }
    Ok(Self { filters })
  }

  pub async fn process(
    &self,
    ctx: &FilterContext,
    feed: &mut Feed,
  ) -> Result<()> {
    self.process_partial(feed, ctx, self.filters.len()).await
  }

  pub async fn process_partial(
    &self,
    feed: &mut Feed,
    ctx: &FilterContext,
    limit_filters: usize,
  ) -> Result<()> {
    for filter in self.filters.iter().take(limit_filters) {
      filter.run(ctx, feed).await?;
    }
    Ok(())
  }
}

macro_rules! define_filters {
  ($($variant:ident => $config:ty);* ;) => {
    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(rename_all = "snake_case")]
    pub enum FilterConfig {
      $(
        $variant($config),
      )*
    }

    impl FilterConfig {
      // currently only used in tests
      #[cfg(test)]
      pub fn parse_yaml(input: &str) -> Result<Box<dyn std::any::Any>> {
        #[derive(Deserialize)]
        struct Dummy {
          #[serde(flatten)]
          config: FilterConfig
        }

        use crate::util::ConfigError;
        let config: Dummy = serde_yaml::from_str(input).map_err(ConfigError::from)?;
        match config.config {
          $(FilterConfig::$variant(config) => {
            Ok(Box::new(config))
          })*
        }
      }

      pub async fn build(self) -> Result<BoxedFilter> {
        match self {
          $(FilterConfig::$variant(config) => {
            let filter = config.build().await?;
            Ok(BoxedFilter::from(filter))
          })*
        }
      }
    }
  }
}

define_filters!(
  Js => js::JsConfig;
  FullText => full_text::FullTextConfig;
  SimplifyHtml => simplify_html::SimplifyHtmlConfig;
  RemoveElement => html::RemoveElementConfig;
  KeepElement => html::KeepElementConfig;
  Split => html::SplitConfig;
  Sanitize => sanitize::SanitizeConfig;
  KeepOnly => select::KeepOnlyConfig;
  Discard => select::DiscardConfig;
  Highlight => highlight::HighlightConfig;
  Merge => merge::MergeConfig;
);
