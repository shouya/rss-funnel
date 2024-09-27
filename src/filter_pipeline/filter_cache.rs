use std::time::Duration;

use futures::Future;

use crate::{
  feed::{Feed, NormalizedFeed, NormalizedPost, Post},
  util::TimedLruCache,
  Result,
};

pub struct FilterCache {
  feed_cache: TimedLruCache<NormalizedFeed, Feed>,
  post_cache: TimedLruCache<NormalizedPost, Post>,
}

impl FilterCache {
  pub fn new() -> Self {
    Self {
      feed_cache: TimedLruCache::new(1, Duration::from_secs(12 * 3600)),
      post_cache: TimedLruCache::new(40, Duration::from_secs(3600)),
    }
  }

  pub async fn run<F, Fut>(&self, mut input_feed: Feed, f: F) -> Result<Feed>
  where
    F: FnOnce(Feed) -> Fut,
    Fut: Future<Output = Result<Feed>>,
  {
    let input_feed_norm = input_feed.normalize();
    // feed-level cache
    if let Some(output_feed) = self.feed_cache.get_cached(&input_feed_norm) {
      return Ok(output_feed);
    }

    // post-level cache
    let all_posts = input_feed.take_posts();
    let mut final_output_posts = Vec::new();
    let mut uncached_input_posts = Vec::new();
    let mut uncached_input_post_norms = Vec::new();

    for (post_norm, post) in input_feed_norm.posts.iter().zip(all_posts) {
      if let Some(post) = self.post_cache.get_cached(post_norm) {
        final_output_posts.push(Some(post));
      } else {
        final_output_posts.push(None);
        uncached_input_posts.push(post);
        uncached_input_post_norms.push(post_norm.clone());
      }
    }

    input_feed.set_posts(uncached_input_posts);
    let mut output_feed = f(input_feed).await?;
    let mut output_posts = output_feed.take_posts();
    self.register_posts(uncached_input_post_norms, output_posts.clone());

    // assemble the final feed in order
    output_posts.reverse();
    for post in &mut final_output_posts {
      if post.is_none() {
        *post = output_posts.pop();
      }
    }
    let final_output_posts = final_output_posts.into_iter().flatten().collect();
    output_feed.set_posts(final_output_posts);

    self.register_feed(input_feed_norm.clone(), output_feed.clone());
    Ok(output_feed)
  }

  fn register_posts(
    &self,
    input_posts: Vec<NormalizedPost>,
    output_posts: Vec<Post>,
  ) {
    if input_posts.len() != output_posts.len() {
      tracing::warn!("filter produced different number of posts");
      return;
    }

    for (input_post, output_post) in
      input_posts.into_iter().zip(output_posts.into_iter())
    {
      self.post_cache.insert(input_post, output_post);
    }
  }

  fn register_feed(&self, input_feed: NormalizedFeed, output_feed: Feed) {
    self.feed_cache.insert(input_feed, output_feed);
  }
}
