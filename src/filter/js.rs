use serde::{Deserialize, Serialize};

use crate::feed::Feed;
use crate::js::{Globals, Runtime};
use crate::util::Result;

use super::{FeedFilter, FeedFilterConfig};

#[derive(Serialize, Deserialize)]
pub struct JsConfig {
  /// The javascript code to run
  code: String,
}

pub struct JsFilter {
  code: String,
  runtime: Runtime,
}

#[async_trait::async_trait]
impl FeedFilterConfig for JsConfig {
  type Filter = JsFilter;

  async fn build(&self) -> Result<Self::Filter> {
    let runtime = Runtime::new().await?;
    Ok(Self::Filter {
      code: self.code.clone(),
      runtime,
    })
  }
}

#[async_trait::async_trait]
impl FeedFilter for JsFilter {
  async fn run(&self, feed: &mut Feed) -> Result<()> {
    let mut posts = Vec::new();

    for post in feed.posts.iter() {
      let mut globals = Globals::new();
      globals.set("post", post);

      if self.runtime.eval(&self.code, globals).await? {
        posts.push(post.clone());
      }
    }

    feed.posts = posts;
    Ok(())
  }
}
