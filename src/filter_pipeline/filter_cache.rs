use std::time::Duration;

use futures::Future;

use crate::{
  feed::{Feed, NormalizedFeed, NormalizedPost, Post},
  util::TimedLruCache,
  Result,
};

pub struct FilterCache {
  feed_cache: TimedLruCache<NormalizedFeed, Feed>,
  // WIP
  #[expect(unused)]
  post_cache: TimedLruCache<NormalizedPost, Option<Post>>,
}

impl FilterCache {
  pub fn new() -> Self {
    Self {
      feed_cache: TimedLruCache::new(1, Duration::from_secs(12 * 3600)),
      post_cache: TimedLruCache::new(40, Duration::from_secs(3600)),
    }
  }

  pub async fn run<F, Fut>(&self, feed: Feed, f: F) -> Result<Feed>
  where
    F: FnOnce(Feed) -> Fut,
    Fut: Future<Output = Result<Feed>>,
  {
    let normalized_feed = feed.normalize();
    if let Some(output_feed) = self.feed_cache.get_cached(&normalized_feed) {
      return Ok(output_feed);
    }

    match f(feed).await {
      Ok(output_feed) => {
        self.register(normalized_feed, output_feed.clone());
        Ok(output_feed)
      }
      Err(e) => Err(e),
    }
  }

  fn register(&self, input_feed: NormalizedFeed, output_feed: Feed) {
    self.feed_cache.insert(input_feed, output_feed);
  }
}
