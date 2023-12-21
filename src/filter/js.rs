use serde::{Deserialize, Serialize};

use crate::feed::Feed;
use crate::js::{AsJson, Runtime};
use crate::util::Result;

use super::{FeedFilter, FeedFilterConfig};

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsConfig {
  /// The javascript code to run
  code: String,
}

pub struct JsFilter {
  runtime: Runtime,
}

#[async_trait::async_trait]
impl FeedFilterConfig for JsConfig {
  type Filter = JsFilter;

  async fn build(&self) -> Result<Self::Filter> {
    let runtime = Runtime::new().await?;
    runtime.eval(&self.code).await?;

    Ok(Self::Filter { runtime })
  }
}

#[async_trait::async_trait]
impl FeedFilter for JsFilter {
  async fn run(&self, feed: &mut Feed) -> Result<()> {
    let mut posts = Vec::new();

    for post in feed.posts.iter() {
      if self.runtime.call_fn("keep_post", (AsJson(post),)).await? {
        posts.push(post.clone());
      }
    }

    feed.posts = posts;
    Ok(())
  }
}
