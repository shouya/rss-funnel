use regex::{Regex, RegexSet};
use serde::{Deserialize, Serialize};

use crate::util::{ConfigError, Result, SingleOrVec};

use super::{FeedFilter, FeedFilterConfig};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct KeepOnlyConfig(AnyMatchConfig);

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct DiscardConfig(AnyMatchConfig);

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
enum AnyMatchConfig {
  SingleContains(String),
  MultipleContains(Vec<String>),
  MatchConfig(MatchConfig),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct MatchConfig {
  #[serde(default)]
  matches: SingleOrVec<serde_regex::Serde<Regex>>,
  #[serde(default)]
  contains: SingleOrVec<String>,
  #[serde(default)]
  field: Field,
  #[serde(default)]
  case_sensitive: bool,
}

impl Default for MatchConfig {
  fn default() -> Self {
    Self {
      matches: SingleOrVec::empty(),
      contains: SingleOrVec::empty(),
      field: Field::default(),
      case_sensitive: false,
    }
  }
}

impl AnyMatchConfig {
  fn to_match_config(&self) -> MatchConfig {
    match self {
      Self::SingleContains(s) => MatchConfig {
        contains: SingleOrVec::Vec(vec![s.clone()]),
        ..Default::default()
      },
      Self::MultipleContains(v) => MatchConfig {
        contains: SingleOrVec::Vec(v.clone()),
        ..Default::default()
      },
      Self::MatchConfig(m) => m.clone(),
    }
  }
}

impl MatchConfig {
  fn regexes(&self) -> Vec<String> {
    let mut out = vec![];

    for m in &self.matches {
      out.push(m.as_str().to_string());
    }
    for p in &self.contains {
      out.push(regex::escape(p));
    }

    out
  }

  fn regex_set(&self) -> Result<RegexSet> {
    Ok(RegexSet::new(self.regexes()).map_err(ConfigError::from)?)
  }

  fn to_select(&self, action: Action) -> Result<Select> {
    let needle = self.regex_set()?;
    let field = self.field;

    Ok(Select {
      needle,
      field,
      action,
    })
  }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
enum Field {
  Title,
  Content,
  Any,
}

impl Field {
  fn extract<'a>(&self, post: &'a crate::feed::Post) -> Vec<&'a str> {
    let vec = match self {
      Self::Title => vec![post.title()],
      Self::Content => vec![post.description()],
      Self::Any => {
        vec![post.title(), post.description()]
      }
    };

    vec.into_iter().flatten().collect()
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

  async fn build(self) -> Result<Self::Filter> {
    self.0.to_match_config().to_select(Action::Include)
  }
}

#[async_trait::async_trait]
impl FeedFilterConfig for DiscardConfig {
  type Filter = Select;

  async fn build(self) -> Result<Self::Filter> {
    self.0.to_match_config().to_select(Action::Exclude)
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
  async fn run(&self, feed: &mut crate::feed::Feed) -> Result<()> {
    let posts = feed.take_posts();
    let mut new_posts = vec![];

    for post in posts {
      if self.should_keep(&post) {
        new_posts.push(post);
      }
    }

    feed.set_posts(new_posts);
    Ok(())
  }
}
