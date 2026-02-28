use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{FeedFilter, FeedFilterConfig, FilterContext};
use crate::{error::Result, feed::Feed};

#[derive(
  JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
/// Inject a CSS `<style>` block into the body of each post.
pub struct InjectCssConfig {
  css: String,
}

#[async_trait::async_trait]
impl FeedFilterConfig for InjectCssConfig {
  type Filter = InjectCss;

  async fn build(self) -> Result<Self::Filter> {
    Ok(InjectCss { css: self.css })
  }
}

pub struct InjectCss {
  css: String,
}

#[async_trait::async_trait]
impl FeedFilter for InjectCss {
  async fn run(
    &self,
    _ctx: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    let style_tag = format!("<style>{}</style>", self.css);
    let mut posts = feed.take_posts();
    for post in &mut posts {
      post.modify_bodies(|body| {
        body.insert_str(0, &style_tag);
      });
    }
    feed.set_posts(posts);
    Ok(feed)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test_utils::{assert_filter_parse, fetch_endpoint};

  #[test]
  fn test_config_inject_css() {
    let config = r#"
      inject_css: "body { color: red; }"
    "#;

    let expected = InjectCssConfig {
      css: "body { color: red; }".into(),
    };

    assert_filter_parse(config, expected);
  }

  #[tokio::test]
  async fn test_inject_css_filter() {
    let config = r#"
      !endpoint
      path: /feed.xml
      source: fixture:///sample_atom.xml
      filters:
        - inject_css: "body { font-size: 16px; }"
    "#;

    let mut feed = fetch_endpoint(config, "").await;
    let posts = feed.take_posts();
    assert!(!posts.is_empty());
    for post in &posts {
      for body in post.bodies() {
        assert!(
          body.starts_with("<style>body { font-size: 16px; }</style>"),
          "body should start with style tag, got: {}",
          &body[..body.len().min(80)]
        );
      }
    }
  }
}
