use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::client::{Client, ClientConfig};
use crate::feed::Feed;
use crate::source::{Source, SourceConfig};
use crate::util::Result;

use super::{
  FeedFilter, FeedFilterConfig, FilterConfig, FilterContext, Filters,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum MergeConfig {
  Simple(MergeSimpleConfig),
  Full(MergeFullConfig),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct MergeSimpleConfig {
  source: SourceConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MergeFullConfig {
  source: SourceConfig,
  #[serde(default)]
  client: ClientConfig,
  #[serde(default)]
  filters: Vec<FilterConfig>,
}

impl From<MergeSimpleConfig> for MergeFullConfig {
  fn from(config: MergeSimpleConfig) -> Self {
    Self {
      source: config.source,
      client: ClientConfig::default(),
      filters: Default::default(),
    }
  }
}

impl From<MergeConfig> for MergeFullConfig {
  fn from(config: MergeConfig) -> Self {
    match config {
      MergeConfig::Simple(config) => config.into(),
      MergeConfig::Full(config) => config,
    }
  }
}

#[async_trait::async_trait]
impl FeedFilterConfig for MergeConfig {
  type Filter = Merge;

  async fn build(self) -> Result<Self::Filter> {
    let MergeFullConfig {
      client,
      filters,
      source,
    } = self.into();
    let client = client.build(Duration::from_secs(15 * 60))?;
    let filters = Filters::from_config(filters).await?;
    let source = source.try_into()?;

    Ok(Merge {
      client,
      source,
      filters,
    })
  }
}

pub struct Merge {
  client: Client,
  source: Source,
  filters: Filters,
}

#[async_trait::async_trait]
impl FeedFilter for Merge {
  async fn run(&self, ctx: &FilterContext, feed: &mut Feed) -> Result<()> {
    let mut new_feed = self.source.fetch_feed(Some(&self.client), None).await?;
    self.filters.process(ctx, &mut new_feed).await?;
    feed.merge(new_feed)?;
    Ok(())
  }
}

#[cfg(test)]
mod test {
  use crate::test_utils::fetch_endpoint;
  use std::collections::HashMap;

  #[tokio::test]
  async fn test_merge_filter() {
    let config = r#"
      !endpoint
      path: /feed.xml
      source: fixture:///scishow.xml
      filters:
        - merge:
            source: fixture:///scishow.xml
            filters:
              - js: |
                  function modify_post(feed, post) {
                    post.title.value += " (modified)";
                    return post;
                  }
    "#;

    let mut feed = fetch_endpoint(config, "").await;
    let posts = feed.take_posts();

    // First group posts by url. Then assert, in each group, one title
    // is "modified" of another
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for post in posts {
      let link = post.link().unwrap().into();
      let title = post.title().unwrap().into();
      groups.entry(link).or_default().push(title);
    }

    for (_, titles) in groups {
      assert_eq!(titles.len(), 2);
      assert!(
        titles[0] == format!("{} (modified)", titles[1])
          || titles[1] == format!("{} (modified)", titles[0])
      );
    }
  }
}
