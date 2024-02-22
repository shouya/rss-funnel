use std::borrow::Cow;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::util::Result;
use crate::{feed::Feed, util::ConfigError};

use super::{FeedFilter, FeedFilterConfig, FilterContext};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SanitizeOpReplaceConfig {
  from: String,
  to: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SanitizeOpConfig {
  remove: Option<String>,
  remove_regex: Option<String>,
  replace: Option<SanitizeOpReplaceConfig>,
  replace_regex: Option<SanitizeOpReplaceConfig>,
}

impl SanitizeOpConfig {
  fn into_op(self) -> Result<SanitizeOp> {
    // must ensure that only one of the options is Some
    let num_selected = self.remove.is_some() as u8
      + self.remove_regex.is_some() as u8
      + self.replace.is_some() as u8
      + self.replace_regex.is_some() as u8;
    if num_selected != 1 {
      let message = format!(
        "Exactly one of {}, {}, {}, {} must be specified for `sanitize' filter",
        "remove", "remove_regex", "replace", "replace_regex"
      );
      return Err(ConfigError::Message(message.to_string()).into());
    }

    macro_rules! parse_regex {
      ($regex:expr) => {
        Regex::new(&$regex).map_err(ConfigError::from)?
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
      let from = parse_regex!(regex::escape(&text.from));
      let to = text.to;
      return Ok(SanitizeOp::Replace(from, to));
    }

    if let Some(repl) = self.replace_regex {
      let from = parse_regex!(repl.from);
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

#[derive(Serialize, Deserialize, Clone, Debug)]
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
  fn filter_description(&self, description: &str) -> String {
    let mut description = Cow::Borrowed(description);

    for op in &self.ops {
      let (needle, repl) = match op {
        SanitizeOp::Remove(needle) => (needle, ""),
        SanitizeOp::Replace(needle, repl) => (needle, repl.as_str()),
      };

      if let Cow::Owned(o) = needle.replace_all(&description, repl) {
        description = Cow::Owned(o)
      }
    }

    description.to_string()
  }
}

#[async_trait::async_trait]
impl FeedFilter for Sanitize {
  async fn run(&self, _ctx: &mut FilterContext, feed: &mut Feed) -> Result<()> {
    let mut posts = feed.take_posts();
    for post in &mut posts {
      if let Some(description) = post.description_mut() {
        *description = self.filter_description(description);
      }
    }

    feed.set_posts(posts);
    Ok(())
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
          replace: Some(SanitizeOpReplaceConfig {
            from: "bar".into(),
            to: "baz".into(),
          }),
          replace_regex: None,
        },
        SanitizeOpConfig {
          remove: None,
          remove_regex: None,
          replace: None,
          replace_regex: Some(SanitizeOpReplaceConfig {
            from: r"\w+".into(),
            to: "qux".into(),
          }),
        },
      ],
    };

    assert_filter_parse(config, expected);
  }
}
