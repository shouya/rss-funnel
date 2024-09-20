use std::time::Duration;

use futures::{stream, StreamExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::client::{Client, ClientConfig};
use crate::feed::Feed;
use crate::filter_pipeline::{FilterPipeline, FilterPipelineConfig};
use crate::source::{SimpleSourceConfig, Source};
use crate::util::{ConfigError, Error, Result, SingleOrVec};

use super::{FeedFilter, FeedFilterConfig, FilterContext};

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(untagged)]
pub enum MergeConfig {
  /// Simple merge with default client and no filters
  Simple(MergeSimpleConfig),
  /// Fully customized merge
  Full(MergeFullConfig),
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
pub struct MergeSimpleConfig {
  pub source: SingleOrVec<SimpleSourceConfig>,
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct MergeFullConfig {
  /// Source configuration
  pub source: SingleOrVec<SimpleSourceConfig>,
  /// Number of concurrent requests to make for fetching multiple sources (default: 20)
  #[serde(default)]
  pub parallelism: Option<usize>,
  /// Client configuration
  #[serde(default)]
  pub client: Option<ClientConfig>,
  /// Filters to apply to the merged feed
  #[serde(default)]
  pub filters: Option<FilterPipelineConfig>,
}

impl From<MergeSimpleConfig> for MergeFullConfig {
  fn from(config: MergeSimpleConfig) -> Self {
    Self {
      source: config.source,
      client: Default::default(),
      filters: Default::default(),
      parallelism: None,
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

  async fn build(self) -> Result<Self::Filter, ConfigError> {
    let MergeFullConfig {
      client,
      filters,
      source,
      parallelism,
    } = self.into();
    let client = client
      .unwrap_or_default()
      .build(Duration::from_secs(15 * 60))?;
    let filters = filters.unwrap_or_default().build().await?;
    let sources = source
      .into_vec()
      .into_iter()
      .map(|s| s.try_into())
      .collect::<Result<_, _>>()?;
    let parallelism = parallelism.unwrap_or(20);

    Ok(Merge {
      client,
      sources,
      parallelism,
      filters,
    })
  }
}

pub struct Merge {
  client: Client,
  parallelism: usize,
  sources: Vec<Source>,
  filters: FilterPipeline,
}

impl Merge {
  async fn fetch_sources(
    &self,
    ctx: &FilterContext,
  ) -> Result<(Vec<Feed>, Vec<(Source, Error)>)> {
    let iter = stream::iter(self.sources.clone())
      .map(|source: Source| {
        let client = &self.client;
        async move {
          let fetch_res = source.fetch_feed(ctx, Some(client)).await;
          (source, fetch_res)
        }
      })
      .buffered(self.parallelism)
      .collect::<Vec<_>>()
      .await
      .into_iter();

    collect_partial_oks(iter)
  }
}

#[async_trait::async_trait]
impl FeedFilter for Merge {
  async fn run(&self, ctx: &mut FilterContext, mut feed: Feed) -> Result<Feed> {
    let (new_feeds, errors) = self.fetch_sources(ctx).await?;

    for new_feed in new_feeds {
      let ctx = ctx.subcontext();
      let filtered_new_feed = self.filters.run(ctx, new_feed).await?;
      feed.merge(filtered_new_feed)?;
    }

    for (source, error) in errors {
      let title = "Failed fetching source".to_owned();
      let source_url = source.full_url(ctx).map(|u| u.to_string());
      let source_desc = source_url
        .clone()
        .unwrap_or_else(|| format!("{:?}", source));

      let body = format!("<p>Source: {source_desc}</p><p>Error: {error}</p>");
      feed.add_item(title, body, source_url.unwrap_or_default());
    }

    feed.reorder();
    Ok(feed)
  }
}

// return Err(e) only if all results are Err.
// otherwise return a Vec<T> with failed results
#[allow(clippy::type_complexity)]
fn collect_partial_oks<S, T, E>(
  iter: impl Iterator<Item = (S, Result<T, E>)>,
) -> Result<(Vec<T>, Vec<(S, E)>), E> {
  let mut oks = Vec::new();
  let mut errs = Vec::new();
  for (source, res) in iter {
    match res {
      Ok(ok) => oks.push(ok),
      Err(err) => errs.push((source, err)),
    }
  }

  if oks.is_empty() && !errs.is_empty() {
    Err(errs.pop().map(|(_, e)| e).unwrap())
  } else {
    Ok((oks, errs))
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
      source: fixture:///youtube.xml
      filters:
        - merge:
            source: fixture:///youtube.xml
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

  #[tokio::test]
  async fn test_parallelism() {
    let config = r#"
      !endpoint
      path: /feed.xml
      source: fixture:///youtube.xml
      filters:
      - merge:
        - fixture:///youtube.xml
        - fixture:///youtube.xml
    "#;

    let mut feed = fetch_endpoint(config, "").await;
    let posts = feed.take_posts();

    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for post in posts {
      let link = post.link().unwrap().into();
      let title = post.title().unwrap().into();
      groups.entry(link).or_default().push(title);
    }

    for (_, titles) in groups {
      assert_eq!(titles.len(), 3);
    }
  }

  #[test]
  fn test_partial_collect() {
    let results = vec![
      (1, Ok(1)),
      (2, Err("error2")),
      (3, Ok(3)),
      (4, Err("error4")),
    ];
    let (oks, errs) = super::collect_partial_oks(results.into_iter()).unwrap();
    assert_eq!(oks, vec![1, 3]);
    assert_eq!(errs, vec![(2, "error2"), (4, "error4")]);

    let results = vec![(1, Err::<(), _>("error1")), (2, Err("error2"))];
    let err = super::collect_partial_oks(results.into_iter()).unwrap_err();
    assert_eq!(err, "error1");
  }
}
