use std::borrow::Cow;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::util::Result;
use crate::{feed::Feed, util::ConfigError};

use super::{FeedFilter, FeedFilterConfig};

#[derive(Serialize, Deserialize)]
pub struct RegexRemoveConfig {
  patterns: Vec<String>,
}

pub struct RegexRemove {
  patterns: Vec<Regex>,
}

#[async_trait::async_trait]
impl FeedFilterConfig for RegexRemoveConfig {
  type Filter = RegexRemove;

  async fn build(&self) -> Result<Self::Filter> {
    let mut patterns = vec![];
    for pattern in &self.patterns {
      patterns.push(Regex::new(&pattern).map_err(ConfigError::from)?);
    }
    Ok(RegexRemove { patterns })
  }
}

impl RegexRemove {
  fn filter_content(&self, content: &str) -> String {
    let mut content = Cow::Borrowed(content);

    for pattern in &self.patterns {
      match pattern.replace_all(&content, "") {
        Cow::Owned(o) => content = Cow::Owned(o),
        Cow::Borrowed(_) => {} // content unchanged, no need to assign
      }
    }

    content.to_string()
  }
}

#[async_trait::async_trait]
impl FeedFilter for RegexRemove {
  async fn run(&self, feed: &mut Feed) -> Result<()> {
    for post in &mut feed.posts {
      post.description = self.filter_content(&post.description);
    }

    Ok(())
  }
}

#[derive(Serialize, Deserialize)]
pub struct RegexReplaceConfig {
  pattern: String,
  replacement: String,
}

pub struct RegexReplace {
  pattern: Regex,
  replacement: String,
}

#[async_trait::async_trait]
impl FeedFilterConfig for RegexReplaceConfig {
  type Filter = RegexReplace;

  async fn build(&self) -> Result<Self::Filter> {
    let pattern = Regex::new(&self.pattern).map_err(ConfigError::from)?;
    let replacement = self.replacement.clone();

    Ok(RegexReplace {
      pattern,
      replacement,
    })
  }
}

#[async_trait::async_trait]
impl FeedFilter for RegexReplace {
  async fn run(&self, feed: &mut Feed) -> Result<()> {
    for post in &mut feed.posts {
      post.description = self
        .pattern
        .replace_all(&post.description, &self.replacement)
        .to_string();
    }

    Ok(())
  }
}
