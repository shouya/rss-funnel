use std::time::Duration;

use duration_str::deserialize_duration;
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::feed::{Feed, Post};
use crate::util::Result;

use super::{FeedFilter, FeedFilterConfig};

const DEFAULT_PARALLELISM: usize = 20;

#[derive(Serialize, Deserialize)]
pub struct FullTextConfig {
  #[serde(default = "default_timeout")]
  #[serde(deserialize_with = "deserialize_duration")]
  timeout: Duration,
  parallelism: Option<usize>,
}

pub struct FullTextFilter {
  client: reqwest::Client,
  parallelism: usize,
}

#[async_trait::async_trait]
impl FeedFilterConfig for FullTextConfig {
  type Filter = FullTextFilter;

  async fn build(&self) -> Result<Self::Filter> {
    let client = reqwest::Client::builder()
      .user_agent(crate::util::USER_AGENT)
      .tcp_keepalive(Some(self.timeout))
      .timeout(self.timeout)
      .build()?;
    let parallelism = self.parallelism.unwrap_or(DEFAULT_PARALLELISM);

    Ok(FullTextFilter {
      client,
      parallelism,
    })
  }
}

impl FullTextFilter {
  async fn try_fetch_full_post(&self, post: &mut Post) -> Result<()> {
    let resp = self.client.get(&post.link).send().await?;
    let resp = resp.error_for_status()?;
    post.description = resp.text().await?;
    Ok(())
  }

  async fn fetch_full_post(&self, mut post: Post) -> Result<Post> {
    // if anything went wrong when fetching the full text, we simply
    // append the error message to the description instead of failing
    // completely.
    match self.try_fetch_full_post(&mut post).await {
      Ok(_) => Ok(post),
      Err(e) => {
        let message = format!("\n<br>\n<br>\nerror fetching full text: {}", e);
        post.description.push_str(&message);
        Ok(post)
      }
    }
  }

  async fn fetch_all_posts(&self, posts: Vec<Post>) -> Result<Vec<Post>> {
    stream::iter(posts)
      .map(|post| self.fetch_full_post(post))
      .buffered(self.parallelism)
      .collect::<Vec<_>>()
      .await
      .into_iter()
      .collect::<Result<Vec<_>>>()
  }
}

#[async_trait::async_trait]
impl FeedFilter for FullTextFilter {
  async fn run(&self, feed: &mut Feed) -> Result<()> {
    let posts = feed.posts.split_off(0);
    let posts = self.fetch_all_posts(posts).await?;
    feed.posts = posts;
    Ok(())
  }
}

fn default_timeout() -> Duration {
  Duration::from_secs(10)
}
