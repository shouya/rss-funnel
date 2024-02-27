mod full_text;
mod highlight;
mod html;
mod js;
mod merge;
mod note;
mod sanitize;
mod select;
mod simplify_html;

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{feed::Feed, util::Result};

#[derive(Clone)]
pub struct FilterContext {
  limit_filters: Option<usize>,
  /// The base URL of the application. Used to construct absolute URLs
  /// from a relative path.
  base: Option<Url>,
}

impl FilterContext {
  pub fn new() -> Self {
    Self {
      limit_filters: None,
      base: None,
    }
  }

  pub fn limit_filters(&self) -> Option<usize> {
    self.limit_filters
  }

  pub fn base(&self) -> Option<&Url> {
    self.base.as_ref()
  }

  pub fn set_limit_filters(&mut self, limit: usize) {
    self.limit_filters = Some(limit);
  }

  pub fn set_base(&mut self, base: Url) {
    self.base = Some(base);
  }

  pub fn subcontext(&self) -> Self {
    Self {
      limit_filters: None,
      base: self.base.clone(),
    }
  }
}

#[async_trait::async_trait]
pub trait FeedFilter {
  async fn run(&self, ctx: &mut FilterContext, feed: Feed) -> Result<Feed>;
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
  async fn run(&self, ctx: &mut FilterContext, feed: Feed) -> Result<Feed> {
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

macro_rules! define_filters {
  ($($variant:ident => $config:ty, $desc:literal);* ;) => {
    paste::paste! {
      #[derive(JsonSchema, Serialize, Deserialize, Clone, Debug)]
      #[serde(rename_all = "snake_case")]
      pub enum FilterConfig {
        $(
           #[doc = "# " $variant:snake "\n\n" $desc "\n"]
           $variant($config),
        )*
      }
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

      pub fn schema() -> schemars::schema::RootSchema {
        let settings = schemars::gen::SchemaSettings::draft07().with(|s| {
          s.option_nullable = true;
          s.option_add_null_type = false;
        });
        let gen = settings.into_generator();
        gen.into_root_schema_for::<Self>()
      }
    }
  }
}

define_filters!(
  Js => js::JsConfig, "Run JavaScript code to transform the feed";
  ModifyPost => js::ModifyPostConfig, "Run JavaScript code to modify each post";
  ModifyFeed => js::ModifyFeedConfig, "Run JavaScript code to modify the feed";
  FullText => full_text::FullTextConfig, "Fetch full text content";
  SimplifyHtml => simplify_html::SimplifyHtmlConfig, "Simplify HTML content";
  RemoveElement => html::RemoveElementConfig, "Remove HTML elements";
  KeepElement => html::KeepElementConfig, "Keep only HTML elements";
  Split => html::SplitConfig, "Split each article into multiple articles";
  Sanitize => sanitize::SanitizeConfig, "Redact or replace text";
  KeepOnly => select::KeepOnlyConfig, "Keep only posts matching a condition";
  Discard => select::DiscardConfig, "Discard posts matching a condition";
  Highlight => highlight::HighlightConfig, "Highlight text or pattern";
  Merge => merge::MergeConfig, "Merge extra feed into the main feed";
  Note => note::NoteFilterConfig, "Add non-functional comment";
);
