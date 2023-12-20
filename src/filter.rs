mod full_text;
mod html;
mod js;
mod sanitize;
mod simplify_html;

use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{feed::Feed, util::Result};

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

macro_rules! define_filters {
  ($($variant:ident => $config:ty);* ;) => {
    #[derive(Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum FilterConfig {
      $(
        $variant($config),
      )*
    }

    impl FilterConfig {
      pub async fn build(&self) -> Result<BoxedFilter> {
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
  Sanitize => sanitize::SanitizeConfig;
);
