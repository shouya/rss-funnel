use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
  client::Client,
  feed::{Feed, FeedFormat},
  util::{ConfigError, Error, Result},
};

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(untagged)]
/// # Feed source
pub enum SourceConfig {
  /// # Simple source
  ///
  /// A source that is a simple URL. A relative path (e.g. "/feed.xml")
  /// points to the current instance.
  Simple(String),
  /// # From scratch
  ///
  /// A source that is created from scratch
  FromScratch(FromScratch),
}

#[derive(Clone, Debug)]
pub enum Source {
  AbsoluteUrl(Url),
  RelativeUrl(String),
  FromScratch(FromScratch),
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct FromScratch {
  /// The format of the feed
  pub format: FeedFormat,
  /// The title of the feed
  pub title: String,
  /// The url to the website
  pub link: Option<String>,
  /// A description of the feed
  pub description: Option<String>,
}

impl From<Url> for Source {
  fn from(url: Url) -> Self {
    Source::AbsoluteUrl(url)
  }
}

impl TryFrom<SourceConfig> for Source {
  type Error = ConfigError;

  fn try_from(config: SourceConfig) -> Result<Self, Self::Error> {
    match config {
      SourceConfig::Simple(url) if url.starts_with('/') => {
        Ok(Source::RelativeUrl(url))
      }
      SourceConfig::Simple(url) => {
        let url = Url::parse(&url)?;
        Ok(Source::AbsoluteUrl(url))
      }
      SourceConfig::FromScratch(config) => Ok(Source::FromScratch(config)),
    }
  }
}

impl Source {
  pub async fn fetch_feed(
    &self,
    client: Option<&Client>,
    base: Option<&Url>,
  ) -> Result<Feed> {
    if let Source::FromScratch(config) = self {
      let feed = Feed::from(config);
      return Ok(feed);
    }

    let client =
      client.ok_or_else(|| Error::Message("client not set".into()))?;
    let source_url = match self {
      Source::AbsoluteUrl(url) => url.clone(),
      Source::RelativeUrl(path) => {
        let base =
          base.ok_or_else(|| Error::Message("base_url not set".into()))?;
        base.join(path)?
      }
      Source::FromScratch(_) => unreachable!(),
    };

    client.fetch_feed(&source_url).await
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[tokio::test]
  async fn test_fetch_feed_from_scratch_rss() {
    const YAML_CONFIG: &str = r#"
format: rss
title: "Test Feed"
link: "https://example.com"
description: "A test feed"
"#;

    let config: SourceConfig = serde_yaml::from_str(YAML_CONFIG).unwrap();
    let source = Source::try_from(config).unwrap();
    let feed: Feed = source.fetch_feed(None, None).await.unwrap();
    assert_eq!(feed.title(), "Test Feed");
    assert_eq!(feed.format(), FeedFormat::Rss);
  }

  #[tokio::test]
  async fn test_fetch_feed_from_scratch_atom() {
    const YAML_CONFIG: &str = r#"
format: atom
title: "Test Feed"
link: "https://example.com"
description: "A test feed"
"#;

    let config: SourceConfig = serde_yaml::from_str(YAML_CONFIG).unwrap();
    let source = Source::try_from(config).unwrap();
    let feed: Feed = source.fetch_feed(None, None).await.unwrap();
    assert_eq!(feed.title(), "Test Feed");
    assert_eq!(feed.format(), FeedFormat::Atom);
  }
}
