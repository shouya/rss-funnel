use std::time::Duration;

use futures::Future;

use crate::{
  feed::{Feed, NormalizedFeed, NormalizedPost, Post},
  util::TimedLruCache,
  Result,
};

pub struct FilterCache {
  feed_cache: FeedCache,
  post_cache: PostCache,
}

impl FilterCache {
  pub fn new() -> Self {
    Self {
      feed_cache: FeedCache {
        cache: TimedLruCache::new(1, Duration::from_secs(12 * 3600)),
      },
      post_cache: PostCache {
        cache: TimedLruCache::new(40, Duration::from_secs(3600)),
      },
    }
  }

  pub async fn run<F, Fut>(&self, feed: Feed, f: F) -> Result<Feed>
  where
    F: FnOnce(Feed) -> Fut,
    Fut: Future<Output = Result<Feed>>,
  {
    f(feed).await
  }
}

struct PostCache {
  // input -> output
  cache: TimedLruCache<NormalizedPost, Post>,
}

struct FeedCache {
  // input -> output
  cache: TimedLruCache<NormalizedFeed, Feed>,
}
