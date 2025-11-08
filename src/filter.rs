pub(crate) mod convert;
pub(crate) mod full_text;
pub(crate) mod highlight;
pub(crate) mod html;
pub(crate) mod image_proxy;
pub(crate) mod js;
pub(crate) mod json_to_feed;
pub(crate) mod limit;
pub(crate) mod magnet;
pub(crate) mod merge;
pub(crate) mod note;
pub(crate) mod sanitize;
pub(crate) mod select;
pub(crate) mod simplify_html;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{formats::CommaSeparator, serde_as, StringWithSeparator};
use url::Url;

use crate::{
  feed::Feed, filter_cache::CacheGranularity, ConfigError, Error, Result,
};

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(transparent)]
pub struct FilterSkip {
  #[serde_as(as = "StringWithSeparator::<CommaSeparator, usize>")]
  indices: HashSet<usize>,
}

impl FilterSkip {
  pub(crate) fn upto(n: usize) -> Self {
    let indices = (0..n).collect::<HashSet<usize>>();
    Self { indices }
  }

  pub fn allows_filter(&self, index: usize) -> bool {
    !self.indices.contains(&index)
  }
}

#[derive(Clone)]
pub struct FilterContext {
  /// The base URL of the application. Used to construct absolute URLs
  /// from a relative path.
  base: Option<Url>,

  /// User supplied source (`?source=` query parameter)
  source: Option<Url>,

  /// The maximum number of filters to run on this pipeline
  filter_skip: Option<FilterSkip>,

  /// The extra query parameters passed to the endpoint
  extra_queries: HashMap<String, String>,

  /// Logs collected from the filters. None indicates logging is
  /// disabled.
  logs: Option<Vec<String>>,
}

pub struct SubContext<'a> {
  context: &'a mut FilterContext,
  saved_filter_skip: Option<FilterSkip>,
}

impl AsMut<FilterContext> for SubContext<'_> {
  fn as_mut(&mut self) -> &mut FilterContext {
    self.context
  }
}

impl Drop for SubContext<'_> {
  fn drop(&mut self) {
    self.context.filter_skip = self.saved_filter_skip.take();
  }
}

impl FilterContext {
  #[cfg(test)]
  pub fn new() -> Self {
    Self {
      base: None,
      filter_skip: None,
      source: None,
      extra_queries: HashMap::new(),
      logs: None,
    }
  }

  pub fn base(&self) -> Option<&Url> {
    self.base.as_ref()
  }

  pub fn base_expected(&self) -> Result<&Url> {
    self.base().ok_or_else(|| Error::BaseUrlNotInferred)
  }

  pub fn extra_queries(&self) -> &HashMap<String, String> {
    &self.extra_queries
  }

  pub fn source(&self) -> Option<&Url> {
    self.source.as_ref()
  }

  #[cfg(test)]
  pub fn set_base(&mut self, base: Url) {
    self.base = Some(base);
  }

  pub fn subcontext(&mut self) -> SubContext<'_> {
    let saved_filter_skip = self.filter_skip.take();
    SubContext {
      context: self,
      saved_filter_skip,
    }
  }

  pub fn log<'a, S>(&mut self, msg: S)
  where
    S: Into<std::borrow::Cow<'a, str>>,
  {
    if let Some(logs) = &mut self.logs {
      logs.push(msg.into().into_owned());
    }
  }

  pub fn enable_logging(&mut self) {
    if self.logs.is_none() {
      self.logs = Some(Vec::new());
    }
  }

  pub fn logs(&self) -> Option<&[String]> {
    self.logs.as_deref()
  }

  pub fn from_param(param: &crate::server::EndpointParam) -> Self {
    Self {
      base: param.base().cloned(),
      source: param.source().cloned(),
      filter_skip: param.filter_skip().cloned(),
      extra_queries: param.extra_queries().clone(),
      logs: None,
    }
  }

  pub fn allows_filter(&self, index: usize) -> bool {
    if let Some(f) = &self.filter_skip {
      f.allows_filter(index)
    } else {
      true
    }
  }
}

#[async_trait::async_trait]
pub trait FeedFilter {
  async fn run(&self, ctx: &mut FilterContext, feed: Feed) -> Result<Feed>;

  fn cache_granularity(&self) -> CacheGranularity {
    CacheGranularity::FeedOnly
  }
}

#[async_trait::async_trait]
pub trait FeedFilterConfig {
  type Filter: FeedFilter;

  async fn build(self) -> Result<Self::Filter, ConfigError>;
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
      #[derive(JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
      #[serde(rename_all = "snake_case")]
      #[allow(clippy::large_enum_variant)]
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
      pub fn parse_yaml_variant(input: &str) -> Result<Box<dyn std::any::Any>> {
        let config: FilterConfig = Self::parse_yaml(input)?;
        match config {
          $(FilterConfig::$variant(config) => {
            Ok(Box::new(config))
          })*
        }
      }

      #[cfg(test)]
      pub fn parse_yaml(input: &str) -> Result<Self, ConfigError> {
        #[derive(Deserialize)]
        struct Dummy {
          #[serde(flatten)]
          config: FilterConfig
        }

        let dummy: Dummy = serde_yaml::from_str(input).map_err(ConfigError::from)?;
        Ok(dummy.config)
      }

      pub fn parse_yaml_value(key: &str, value: serde_yaml::Value) -> Result<Self, ConfigError> {
        #[derive(Deserialize)]
        struct Dummy {
          #[serde(flatten)]
          config: FilterConfig
        }

        use serde_yaml::{Value, Mapping, value::{Tag, TaggedValue}};
        let tag = Tag::new(key);
        let key = Value::String(key.to_string());
        let mut mapping = Mapping::new();
        mapping.insert(key, value);
        let yaml_value = Value::Tagged(Box::new(TaggedValue {
          tag,
          value: Value::Mapping(mapping),
        }));

        let dummy: Dummy = serde_yaml::from_value(yaml_value).map_err(ConfigError::from)?;
        Ok(dummy.config)
      }

      pub async fn build(self) -> Result<BoxedFilter, ConfigError> {
        match self {
          $(FilterConfig::$variant(config) => {
            let filter = config.build().await?;
            Ok(BoxedFilter::from(filter))
          })*
        }
      }

      pub fn to_yaml(&self) -> Result<String, ConfigError> {
        Ok(serde_yaml::to_string(self)?)
      }

      pub fn name(&self) -> &'static str {
        match self {
          $(FilterConfig::$variant(_) => paste::paste! {stringify!([<$variant:snake>])},)*
        }
      }

      pub fn is_valid_key(name: &str) -> bool {
        match name {
          $(paste::paste! {stringify!([<$variant:snake>])} => true,)*
          _ => false,
        }
      }

      pub fn schema_for_all() -> HashMap<String, schemars::schema::RootSchema> {
        let settings = schemars::gen::SchemaSettings::draft07().with(|s| {
          s.option_nullable = true;
          s.option_add_null_type = false;
          s.inline_subschemas = true;
        });

        [
          $(
            (
              paste::paste! { stringify!([<$variant:snake>]) }.to_string(),
              settings.clone().into_generator().into_root_schema_for::<$config>()
            ),
          )*
        ].into()
      }

      pub fn schema_for(filter: &str) -> Option<schemars::schema::RootSchema> {
        let settings = schemars::gen::SchemaSettings::draft07().with(|s| {
          s.option_nullable = true;
          s.option_add_null_type = false;
          s.inline_subschemas = true;
        });
        let gen = settings.into_generator();
        match filter {
          $(paste::paste! { stringify!([<$variant:snake>]) } => {
            Some(gen.into_root_schema_for::<$config>())
          })*
          _ => None,
        }
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
  JsonToFeed => json_to_feed::JsonToFeedConfig, "Build posts from JSON data";
  Merge => merge::MergeConfig, "Merge extra feed into the main feed";
  Note => note::NoteFilterConfig, "Add non-functional comment";
  ConvertTo => convert::ConvertToConfig, "Convert feed to another format";
  Limit => limit::LimitConfig, "Limit the number of posts";
  Magnet => magnet::MagnetConfig, "Find magnet links in posts";
  ImageProxy => image_proxy::Config, "Find magnet links in posts";
);
