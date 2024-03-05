use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
  feed::{Feed, FeedFormat},
  util::{ConfigError, Result},
};

use super::{FeedFilter, FeedFilterConfig, FilterContext};

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
/// Convert feed to another format
pub struct ConvertToConfig {
  format: FeedFormat,
}

pub struct ConvertTo {
  format: FeedFormat,
}

#[async_trait::async_trait]
impl FeedFilterConfig for ConvertToConfig {
  type Filter = ConvertTo;

  async fn build(self) -> Result<Self::Filter, ConfigError> {
    Ok(ConvertTo {
      format: self.format,
    })
  }
}

#[async_trait::async_trait]
impl FeedFilter for ConvertTo {
  async fn run(&self, _ctx: &mut FilterContext, feed: Feed) -> Result<Feed> {
    Ok(feed.into_format(self.format))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_utils::fetch_endpoint;
  use crate::util::Result;

  #[tokio::test]
  async fn test_convert_to() -> Result<()> {
    let config = r#"
      !endpoint
      path: /feed.xml
      source: fixture:///minimal_rss_20.xml
      filters:
        - convert_to: atom
    "#;

    let feed = fetch_endpoint(config, "").await;
    assert_eq!(feed.format(), FeedFormat::Atom);

    let feed: atom_syndication::Feed = feed.try_into().unwrap();

    assert_eq!(feed.title.as_str(), "Test");
    assert_eq!(feed.links[0].href, "http://example.com");
    assert_eq!(
      feed.subtitle.as_ref().map(|e| e.as_str()),
      Some("Test description")
    );
    assert_eq!(feed.entries.len(), 1);
    let post = feed.entries.into_iter().next().unwrap();
    assert_eq!(post.title.as_str(), "Item 1");
    assert_eq!(
      post.links.iter().map(|l| &l.href).collect::<Vec<_>>(),
      vec!["http://example.com/item1"]
    );
    assert_eq!(
      post.summary.as_ref().map(|s| s.as_str()),
      Some("Item 1 description")
    );
    Ok(())
  }

  #[tokio::test]
  async fn test_rss_to_atom_to_rss() {
    let config_1 = r#"
      !endpoint
      path: /feed.xml
      source: fixture:///minimal_rss_20.xml
      filters:
        - convert_to: atom
        - convert_to: rss
    "#;
    let config_2 = r#"
      !endpoint
      path: /feed.xml
      source: fixture:///minimal_rss_20.xml
      filters: []
    "#;

    let feed_1 = fetch_endpoint(config_1, "").await;
    let feed_2 = fetch_endpoint(config_2, "").await;
    assert_eq!(feed_1, feed_2);
  }
}
