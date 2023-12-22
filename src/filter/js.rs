use serde::{Deserialize, Serialize};

use crate::feed::Feed;
use crate::js::{AsJson, Runtime};
use crate::util::{Error, Result};

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
    use either::Either::{Left, Right};
    use rquickjs::{Null, Undefined};

    let posts = feed.posts.split_off(0);
    let mut output = Vec::new();

    for post in posts.into_iter() {
      let args = (AsJson(&*feed), AsJson(&post));

      match self.runtime.call_fn("update_post", args).await? {
        Left(Left(Null)) => {
          // returning null means the post should be removed
        }
        Left(Right(Undefined)) => {
          return Err(Error::Message(
            "update_post must return the modified post or null".into(),
          ));
        }
        Right(AsJson(updated_post)) => {
          // The merge is needed because there are properties that are
          // not serializable in the javascript.
          output.push(post.merge(updated_post));
        }
      }
    }

    feed.posts = output;
    Ok(())
  }
}
