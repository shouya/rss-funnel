use js_sandbox::Script;
use serde::Serialize;

use super::FeedFilter;
use crate::{
  feed::{Feed, Post},
  util::Error,
};

#[derive(Serialize)]
struct JsFilter {
  #[serde(skip)]
  script: Script,
  source: String,
}

#[derive(Serialize)]
struct JsContext<'a> {
  feed: &'a Feed,
  post: &'a Post,
}

impl FeedFilter for JsFilter {
  fn keep_post(&mut self, feed: &Feed, post: &Post) -> Result<bool, Error> {
    self.script.call("filter", &JsContext { feed, post })
  }
}
