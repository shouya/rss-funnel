#![allow(deprecated)]

use regex::{RegexSet, RegexSetBuilder};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use std::borrow::Cow;

use crate::{feed::Feed, util::SingleOrVec, ConfigError, Result};

use super::{FeedFilter, FeedFilterConfig, FilterContext};

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
/// Keep only posts that match the given criteria
pub struct KeepOnlyConfig(AnyMatchConfig);

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
/// Discard posts that match the given criteria
pub struct DiscardConfig(AnyMatchConfig);

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(untagged)]
enum AnyMatchConfig {
  /// Matches posts containing the given string
  SingleContains(String),
  /// Matches posts containing any of the given strings
  MultipleContains(Vec<String>),
  /// Full match configuration
  MatchConfig(MatchConfig),
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default,
)]
struct MatchConfig {
  /// Regular expression(s) to match
  #[serde(default)]
  matches: SingleOrVec<String>,

  /// String(s) to match
  #[serde(default)]
  contains: SingleOrVec<String>,

  /// Field to match against
  #[serde(default)]
  field: Field,

  /// Whether to match case sensitively
  #[serde(default)]
  case_sensitive: Option<bool>,
}

impl AnyMatchConfig {
  fn into_match_config(self) -> MatchConfig {
    match self {
      Self::SingleContains(s) => MatchConfig {
        contains: SingleOrVec::Vec(vec![s]),
        ..Default::default()
      },
      Self::MultipleContains(v) => MatchConfig {
        contains: SingleOrVec::Vec(v),
        ..Default::default()
      },
      Self::MatchConfig(m) => m,
    }
  }
}

impl MatchConfig {
  fn regexes(&self) -> Vec<Cow<'_, str>> {
    let mut out = vec![];

    for m in &self.matches {
      out.push(Cow::Borrowed(m.as_str()));
    }
    for p in &self.contains {
      out.push(Cow::Owned(regex::escape(p)));
    }

    out
  }

  fn regex_set(&self) -> Result<RegexSet, ConfigError> {
    let case_sensitive = self.case_sensitive.unwrap_or(false);
    let regex_set = RegexSetBuilder::new(self.regexes())
      .case_insensitive(!case_sensitive)
      .build()
      .map_err(ConfigError::from)?;
    Ok(regex_set)
  }

  fn into_select(self, action: Action) -> Result<Select, ConfigError> {
    let needle = self.regex_set()?;
    let field = self.field;

    Ok(Select {
      needle,
      field,
      action,
    })
  }
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash,
)]
#[serde(rename_all = "snake_case")]
enum Field {
  Title,
  Body,
  #[deprecated(note = "use `body` instead")]
  Content,
  Any,
}

impl Field {
  fn extract<'a>(&self, post: &'a crate::feed::Post) -> Vec<&'a str> {
    match self {
      Self::Title => post.title().into_iter().collect(),
      #[allow(deprecated)]
      Self::Body | Self::Content => post.bodies(),
      Self::Any => post.title().into_iter().chain(post.bodies()).collect(),
    }
  }
}

impl Default for Field {
  fn default() -> Self {
    Self::Any
  }
}

#[derive(Clone, Copy, Debug)]
enum Action {
  Include,
  Exclude,
}

#[async_trait::async_trait]
impl FeedFilterConfig for KeepOnlyConfig {
  type Filter = Select;

  async fn build(self) -> Result<Self::Filter, ConfigError> {
    self.0.into_match_config().into_select(Action::Include)
  }
}

#[async_trait::async_trait]
impl FeedFilterConfig for DiscardConfig {
  type Filter = Select;

  async fn build(self) -> Result<Self::Filter, ConfigError> {
    self.0.into_match_config().into_select(Action::Exclude)
  }
}

#[derive(Clone, Debug)]
pub struct Select {
  needle: RegexSet,
  field: Field,
  action: Action,
}

impl Select {
  fn matches(&self, haystack: &[&str]) -> bool {
    haystack.iter().any(|text| self.needle.is_match(text))
  }

  fn should_keep(&self, post: &crate::feed::Post) -> bool {
    let haystack = self.field.extract(post);
    let matches = self.matches(&haystack);

    match self.action {
      Action::Include => matches,
      Action::Exclude => !matches,
    }
  }
}

#[async_trait::async_trait]
impl FeedFilter for Select {
  async fn run(
    &self,
    _ctx: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    let posts = feed.take_posts();
    let mut new_posts = vec![];

    for post in posts {
      if self.should_keep(&post) {
        new_posts.push(post);
      }
    }

    feed.set_posts(new_posts);
    Ok(feed)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test_utils::{assert_filter_parse, fetch_endpoint};

  #[test]
  fn test_config_keep_only_full() {
    let config = r"
      keep_only:
        matches:
          - '\d+'
          - '\bfoo\b'
        field: title
        case_sensitive: true
    ";

    let expected = KeepOnlyConfig(AnyMatchConfig::MatchConfig(MatchConfig {
      matches: SingleOrVec::Vec(vec![r"\d+".into(), r"\bfoo\b".into()]),
      contains: SingleOrVec::empty(),
      field: Field::Title,
      case_sensitive: Some(true),
    }));

    assert_filter_parse(config, expected);
  }

  #[test]
  fn test_config_keep_only_single() {
    let config = r"
      keep_only: foo
    ";

    let expected = KeepOnlyConfig(AnyMatchConfig::SingleContains("foo".into()));

    assert_filter_parse(config, expected);
  }

  #[test]
  fn test_config_keep_only_multiple() {
    let config = r"
        keep_only:
            - foo
            - bar
        ";

    let expected = KeepOnlyConfig(AnyMatchConfig::MultipleContains(vec![
      "foo".into(),
      "bar".into(),
    ]));

    assert_filter_parse(config, expected);
  }

  #[test]
  fn test_config_discard_full() {
    let config = r"
      discard:
        matches:
          - '\d+'
          - '\bfoo\b'
        field: title
        case_sensitive: true
    ";

    let expected = DiscardConfig(AnyMatchConfig::MatchConfig(MatchConfig {
      matches: SingleOrVec::Vec(vec![r"\d+".into(), r"\bfoo\b".into()]),
      contains: SingleOrVec::empty(),
      field: Field::Title,
      case_sensitive: Some(true),
    }));

    assert_filter_parse(config, expected);
  }

  #[tokio::test]
  async fn test_keep_only_filter() {
    let config = r"
      !endpoint
      path: /feed.xml
      source: fixture:///youtube.xml
      filters:
        - keep_only: ElecTric
    ";

    let mut feed = fetch_endpoint(config, "").await;
    let posts = feed.take_posts();
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].title().unwrap(), "This Crystal Is ELECTRIC");
  }

  #[tokio::test]
  async fn test_keep_only_case_sensitive_filter() {
    let config = r"
      !endpoint
      path: /feed.xml
      source: fixture:///youtube.xml
      filters:
        - keep_only:
            case_sensitive: true
            contains: ElEcT
    ";

    let mut feed = fetch_endpoint(config, "").await;
    let posts = feed.take_posts();
    assert_eq!(posts.len(), 0);
  }
}
