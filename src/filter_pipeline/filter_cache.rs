use crate::{
  feed::{Feed, NormalizedFeed, NormalizedPost, Post},
  util::TimedLruCache,
};

#[derive(Default)]
pub struct FilterCache {
  feed_cache: Option<FeedCache>,
  post_cache: Option<PostCache>,
}

impl FilterCache {}

struct PostCache {
  // input -> output
  cache: TimedLruCache<NormalizedPost, Post>,
}

struct FeedCache {
  // input -> output
  cache: TimedLruCache<NormalizedFeed, Feed>,
}
