use std::borrow::Cow;

use regex::{Regex, RegexBuilder};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::feed::Feed;

use super::{FeedFilter, FeedFilterConfig, FilterContext};

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
struct ReplaceConfig {
  from: String,
  to: String,
  #[serde(default)]
  case_sensitive: Option<bool>,
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct SanitizeOpConfig {
  /// Remove all occurrences of the string
  remove: Option<String>,
  /// Remove all matches of the regex
  remove_regex: Option<String>,
  /// Replace all occurrences of the string
  replace: Option<ReplaceConfig>,
  /// Replace all matches of the regex
  replace_regex: Option<ReplaceConfig>,
}

impl SanitizeOpConfig {
  fn into_op(self) -> Result<SanitizeOp> {
    // must ensure that only one of the options is Some
    let num_selected = u8::from(self.remove.is_some())
      + u8::from(self.remove_regex.is_some())
      + u8::from(self.replace.is_some())
      + u8::from(self.replace_regex.is_some());
    if num_selected != 1 {
      anyhow::bail!(
        "Exactly one of {}, {}, {}, {} must be specified for `sanitize' filter",
        "remove",
        "remove_regex",
        "replace",
        "replace_regex"
      );
    }

    macro_rules! parse_regex {
      ($regex:expr) => {
        RegexBuilder::new(&$regex).case_insensitive(true).build()?
      };
      ($regex:expr, $cs:expr) => {
        RegexBuilder::new(&$regex)
          .case_insensitive(!$cs.unwrap_or(false))
          .build()?
      };
    }

    if let Some(text) = self.remove {
      let escaped = regex::escape(&text);
      return Ok(SanitizeOp::Remove(parse_regex!(escaped)));
    }

    if let Some(repl) = self.remove_regex {
      return Ok(SanitizeOp::Remove(parse_regex!(repl)));
    }

    if let Some(text) = self.replace {
      let case_sensitive = text.case_sensitive;
      let from = parse_regex!(regex::escape(&text.from), case_sensitive);
      let to = text.to;
      return Ok(SanitizeOp::Replace(from, to));
    }

    if let Some(repl) = self.replace_regex {
      let case_sensitive = repl.case_sensitive;
      let from = parse_regex!(repl.from, case_sensitive);
      let to = repl.to;
      return Ok(SanitizeOp::Replace(from, to));
    }

    unreachable!()
  }
}

pub enum SanitizeOp {
  Remove(Regex),
  Replace(Regex, String),
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
pub struct SanitizeConfig {
  ops: Vec<SanitizeOpConfig>,
}

pub struct Sanitize {
  ops: Vec<SanitizeOp>,
}

#[async_trait::async_trait]
impl FeedFilterConfig for SanitizeConfig {
  type Filter = Sanitize;

  async fn build(self) -> Result<Self::Filter> {
    let mut ops = Vec::new();
    for conf in self.ops {
      ops.push(conf.into_op()?);
    }

    Ok(Sanitize { ops })
  }
}

impl Sanitize {
  fn filter_body(&self, body: &mut String) {
    for op in &self.ops {
      let (needle, repl) = match op {
        SanitizeOp::Remove(needle) => (needle, ""),
        SanitizeOp::Replace(needle, repl) => (needle, repl.as_str()),
      };

      if let Cow::Owned(o) = needle.replace_all(body, repl) {
        *body = o;
      }
    }
  }
}

#[async_trait::async_trait]
impl FeedFilter for Sanitize {
  async fn run(
    &self,
    _ctx: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    let mut posts = feed.take_posts();
    for post in &mut posts {
      post.modify_bodies(|body| self.filter_body(body));
    }

    feed.set_posts(posts);
    Ok(feed)
  }

  fn cache_granularity(&self) -> super::CacheGranularity {
    super::CacheGranularity::FeedAndPost
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test_utils::assert_filter_parse;

  #[test]
  fn test_config_sanitize() {
    let config = r#"
      sanitize:
        - remove: "foo"
        - remove_regex: '\d+'
        - replace:
            from: "bar"
            to: "baz"
        - replace_regex:
            from: '\w+'
            to: "qux"
    "#;

    let expected = SanitizeConfig {
      ops: vec![
        SanitizeOpConfig {
          remove: Some("foo".into()),
          remove_regex: None,
          replace: None,
          replace_regex: None,
        },
        SanitizeOpConfig {
          remove: None,
          remove_regex: Some(r"\d+".into()),
          replace: None,
          replace_regex: None,
        },
        SanitizeOpConfig {
          remove: None,
          remove_regex: None,
          replace: Some(ReplaceConfig {
            from: "bar".into(),
            to: "baz".into(),
            case_sensitive: None,
          }),
          replace_regex: None,
        },
        SanitizeOpConfig {
          remove: None,
          remove_regex: None,
          replace: None,
          replace_regex: Some(ReplaceConfig {
            from: r"\w+".into(),
            to: "qux".into(),
            case_sensitive: None,
          }),
        },
      ],
    };

    assert_filter_parse(config, expected);
  }
}
