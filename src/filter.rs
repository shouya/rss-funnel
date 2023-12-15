mod composite;
mod js_filter;

use erased_serde::Serialize;

use crate::{
  feed::{Feed, Post},
  util::Error,
};

trait FeedFilter: Serialize {
  fn keep_post(&mut self, feed: &Feed, post: &Post) -> Result<bool, Error>;
}

impl serde::Serialize for Box<dyn FeedFilter> {
  fn serialize<S: serde::Serializer>(
    &self,
    serializer: S,
  ) -> Result<S::Ok, S::Error> {
    erased_serde::serialize(self, serializer)
  }
}
