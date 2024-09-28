use crate::{
  feed::{Feed, NormalizedFeed, NormalizedPost, Post},
  util::TimedLruCache,
  Result,
};
use futures::Future;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CacheGranularity {
  FeedOnly,
  FeedAndPost,
}

pub struct FilterCache {
  feed_cache: TimedLruCache<NormalizedFeed, Feed>,
  post_cache: TimedLruCache<NormalizedPost, Post>,
}

impl FilterCache {
  pub fn new() -> Self {
    Self {
      feed_cache: TimedLruCache::new(5, Duration::from_secs(12 * 3600)),
      post_cache: TimedLruCache::new(40, Duration::from_secs(3600)),
    }
  }

  pub async fn run<F, Fut>(
    &self,
    input_feed: Feed,
    granularity: CacheGranularity,
    f: F,
  ) -> Result<Feed>
  where
    F: FnOnce(Feed) -> Fut,
    Fut: Future<Output = Result<Feed>>,
  {
    let input_feed_norm = input_feed.normalize();

    // try to get the whole feed from cache first
    if let Some(cached_feed) = self.check_feed_cache(&input_feed_norm) {
      return Ok(cached_feed);
    }

    // decide what to do based on cache granularity
    let (uncached_input_feed, final_output_posts) = match granularity {
      CacheGranularity::FeedOnly => (input_feed.clone(), Vec::new()),
      CacheGranularity::FeedAndPost => {
        self.process_post_cache(input_feed.clone(), &input_feed_norm)
      }
    };

    // apply the filter function to the uncached portion
    let mut output_feed = f(uncached_input_feed.clone()).await?;

    // merge cached and newly processed posts
    if granularity == CacheGranularity::FeedAndPost {
      self.register_post_cache(uncached_input_feed, output_feed.clone());
      output_feed = self.reassemble_feed(output_feed, final_output_posts);
    }

    // update caches
    self.register_feed_cache(input_feed, output_feed.clone());

    Ok(output_feed)
  }

  // quick check: is the whole feed already in our cache?
  fn check_feed_cache(&self, input_feed_norm: &NormalizedFeed) -> Option<Feed> {
    self.feed_cache.get_cached(input_feed_norm)
  }

  // sort out which posts we need to process and which we can grab from cache
  fn process_post_cache(
    &self,
    mut input_feed: Feed,
    input_feed_norm: &NormalizedFeed,
  ) -> (Feed, Vec<Option<Post>>) {
    let all_posts = input_feed.take_posts();
    let mut final_output_posts = Vec::new();
    let mut uncached_input_posts = Vec::new();

    for (post_norm, post) in input_feed_norm.posts.iter().zip(all_posts) {
      if let Some(cached_post) = self.post_cache.get_cached(post_norm) {
        final_output_posts.push(Some(cached_post));
      } else {
        final_output_posts.push(None);
        uncached_input_posts.push(post);
      }
    }

    input_feed.set_posts(uncached_input_posts);
    (input_feed, final_output_posts)
  }

  // assemble feed with cached and newly processed posts, in the correct order
  fn reassemble_feed(
    &self,
    mut output_feed: Feed,
    mut final_output_posts: Vec<Option<Post>>,
  ) -> Feed {
    let mut output_posts = output_feed.take_posts();
    output_posts.reverse();

    for post in &mut final_output_posts {
      if post.is_none() {
        *post = output_posts.pop();
      }
    }

    // add any remaining posts from the output feed
    final_output_posts.extend(output_posts.into_iter().rev().map(Some));

    let final_output_posts = final_output_posts.into_iter().flatten().collect();
    output_feed.set_posts(final_output_posts);
    output_feed
  }

  // add processed posts to the post cache
  fn register_post_cache(&self, mut input_feed: Feed, mut output_feed: Feed) {
    let input_posts = input_feed.take_posts();
    let output_posts = output_feed.take_posts();
    if input_posts.len() != output_posts.len() {
      tracing::warn!("input and output post counts do not match");
    }

    for (input_post, output_post) in input_posts.into_iter().zip(output_posts) {
      let input_post_norm = input_post.normalize();
      self.post_cache.insert(input_post_norm, output_post);
    }
  }

  fn register_feed_cache(&self, input_feed: Feed, output_feed: Feed) {
    let input_feed_norm = input_feed.normalize();
    self.feed_cache.insert(input_feed_norm, output_feed);
  }
}
