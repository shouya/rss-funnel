use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::feed::Feed;
use crate::js::{AsJson, Runtime};
use crate::util::{Error, Result};

use super::{FeedFilter, FeedFilterConfig, FilterContext};

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
/// Either define a function `modify_feed` or `modify_post` to modify the feed or posts respectively.
/// <br><br>
/// See <a href="https://github.com/shouya/rss-funnel/wiki/JavaScript-API" target="_blank">JavaScript API</a>.

pub struct JsConfig {
  code: String,
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
/// JavaScript code for for editing post. Modify `post` variable to update the post or set it to null to delete it.
/// <br><br>
/// See <a href="https://github.com/shouya/rss-funnel/wiki/JavaScript-API" target="_blank">JavaScript API</a>.
/// <br><br>
/// Example: <code>modify_post: post.title += " (modified)";</code>
pub struct ModifyPostConfig {
  code: String,
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
/// JavaScript code for for editing feed. Modify `feed` variable to update the feed.
/// <br><br>
/// See <a href="https://github.com/shouya/rss-funnel/wiki/JavaScript-API" target="_blank">JavaScript API</a>.
/// <br><br>
/// Example: <code>modify_feed: feed.title.value = "Modified Feed";</code>
pub struct ModifyFeedConfig {
  code: String,
}

pub struct JsFilter {
  runtime: Runtime,
}

#[async_trait::async_trait]
impl FeedFilterConfig for JsConfig {
  type Filter = JsFilter;

  async fn build(self) -> Result<Self::Filter> {
    let runtime = Runtime::new().await?;
    runtime.eval(&self.code).await?;

    Ok(Self::Filter { runtime })
  }
}

#[async_trait::async_trait]
impl FeedFilterConfig for ModifyPostConfig {
  type Filter = JsFilter;

  async fn build(self) -> Result<Self::Filter> {
    let code = format!(
      "function modify_post(feed, post) {{ {}; return post; }}",
      self.code
    );
    JsConfig { code }.build().await
  }
}

#[async_trait::async_trait]
impl FeedFilterConfig for ModifyFeedConfig {
  type Filter = JsFilter;

  async fn build(self) -> Result<Self::Filter> {
    let code = format!(
      "function modify_feed(feed) {{ {}; return feed; }}",
      self.code
    );
    JsConfig { code }.build().await
  }
}

impl JsFilter {
  async fn modify_feed(&self, feed: &mut Feed) -> Result<()> {
    use either::Either::{Left, Right};
    use rquickjs::Undefined;

    if !self.runtime.fn_exists("modify_feed").await {
      return Ok(());
    }

    let args = (AsJson(&*feed),);

    match self.runtime.call_fn("modify_feed", args).await? {
      Left(Undefined) => {
        return Err(Error::Message(
          "modify_feed must return the modified feed".into(),
        ));
      }
      Right(AsJson(updated_feed)) => {
        *feed = updated_feed;
      }
    }

    Ok(())
  }

  async fn modify_posts(&self, feed: &mut Feed) -> Result<()> {
    use either::Either::{Left, Right};
    use rquickjs::{Null, Undefined};

    if !self.runtime.fn_exists("modify_post").await {
      return Ok(());
    }

    let mut posts = Vec::new();

    for post in feed.take_posts() {
      let args = (AsJson(&*feed), AsJson(&post));

      match self.runtime.call_fn("modify_post", args).await? {
        Left(Left(Null)) => {
          // returning null means the post should be removed
        }
        Left(Right(Undefined)) => {
          return Err(Error::Message(
            "modify_post must return the modified post or null".into(),
          ));
        }
        Right(AsJson(updated_post)) => {
          posts.push(updated_post);
        }
      }
    }

    feed.set_posts(posts);
    Ok(())
  }
}

#[async_trait::async_trait]
impl FeedFilter for JsFilter {
  async fn run(
    &self,
    _ctx: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    self.modify_feed(&mut feed).await?;
    self.modify_posts(&mut feed).await?;
    Ok(feed)
  }
}

#[cfg(test)]
mod tests {
  use crate::test_utils::fetch_endpoint;

  #[tokio::test]
  async fn test_parse_js_config() {
    let config = r#"
      !endpoint
      path: /feed.xml
      source: fixture:///scishow.xml
      filters:
        - js: |
            function modify_feed(feed) {
              feed.title.value = "Modified Feed";
              return feed;
            }

            function modify_post(feed, post) {
              post.title += " (modified)";
              return post;
            }
    "#;

    let mut feed = fetch_endpoint(config, "").await;
    assert_eq!(feed.title(), "Modified Feed");

    let posts = feed.take_posts();
    for post in posts {
      assert!(post.title().unwrap().ends_with(" (modified)"));
    }
  }

  #[tokio::test]
  async fn test_modify_post_filter() {
    let config = r#"
      !endpoint
      path: /feed.xml
      source: fixture:///scishow.xml
      filters:
        - modify_post: post.title += " (modified)";
    "#;

    let mut feed = fetch_endpoint(config, "").await;
    let posts = feed.take_posts();
    for post in posts {
      assert!(post.title().unwrap().ends_with(" (modified)"));
    }
  }

  #[tokio::test]
  async fn test_modify_feed_filter() {
    let config = r#"
      !endpoint
      path: /feed.xml
      source: fixture:///scishow.xml
      filters:
        - modify_feed: feed.title.value = "Modified Feed";
    "#;

    let feed = fetch_endpoint(config, "").await;
    assert_eq!(feed.title(), "Modified Feed");
  }
}
