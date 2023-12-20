use std::borrow::Cow;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::util::Result;
use crate::{feed::Feed, util::ConfigError};

use super::{FeedFilter, FeedFilterConfig};

#[derive(Serialize, Deserialize)]
struct SanitizeOpReplaceConfig {
  from: String,
  to: String,
}

#[derive(Serialize, Deserialize)]
pub struct SanitizeOpConfig {
  remove: Option<String>,
  remove_regex: Option<String>,
  replace: Option<SanitizeOpReplaceConfig>,
  replace_regex: Option<SanitizeOpReplaceConfig>,
}

impl SanitizeOpConfig {
  fn parse(&self) -> Result<SanitizeOp> {
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

    if let Some(text) = &self.remove {
      let escaped = regex::escape(text);
      return Ok(SanitizeOp::Remove(parse_regex!(escaped)));
    }

    if let Some(repl) = &self.remove_regex {
      return Ok(SanitizeOp::Remove(parse_regex!(repl)));
    }

    if let Some(text) = &self.replace {
      let from = parse_regex!(regex::escape(&text.from));
      let to = text.to.clone();
      return Ok(SanitizeOp::Replace(from, to));
    }

    if let Some(repl) = &self.replace_regex {
      let from = parse_regex!(repl.from);
      let to = repl.to.clone();
      return Ok(SanitizeOp::Replace(from, to));
    }

    unreachable!()
  }
}

pub enum SanitizeOp {
  Remove(Regex),
  Replace(Regex, String),
}

#[derive(Serialize, Deserialize)]
pub struct SanitizeConfig {
  ops: Vec<SanitizeOpConfig>,
}

pub struct Sanitize {
  ops: Vec<SanitizeOp>,
}

#[async_trait::async_trait]
impl FeedFilterConfig for SanitizeConfig {
  type Filter = Sanitize;

  async fn build(&self) -> Result<Self::Filter> {
    let mut ops = Vec::new();
    for op in &self.ops {
      ops.push(op.parse()?);
    }

    Ok(Sanitize { ops })
  }
}

impl Sanitize {
  fn filter_content(&self, content: &str) -> String {
    let mut content = Cow::Borrowed(content);

    for op in &self.ops {
      let (needle, repl) = match op {
        SanitizeOp::Remove(needle) => (needle, ""),
        SanitizeOp::Replace(needle, repl) => (needle, repl.as_str()),
      };

      match needle.replace_all(&content, repl) {
        Cow::Owned(o) => content = Cow::Owned(o),
        Cow::Borrowed(_) => {} // content unchanged, no need to assign
      }
    }

    content.to_string()
  }
}

#[async_trait::async_trait]
impl FeedFilter for Sanitize {
  async fn run(&self, feed: &mut Feed) -> Result<()> {
    for post in &mut feed.posts {
      post.description = self.filter_content(&post.description);
    }

    Ok(())
  }
}
