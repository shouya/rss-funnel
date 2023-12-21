use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::feed::{Feed, Post};
use crate::util::Result;

use super::{FeedFilter, FeedFilterConfig};

#[derive(Serialize, Deserialize)]
pub struct FullTextConfig;

pub struct FullTextFilter {
  client: reqwest::Client,
}

#[async_trait::async_trait]
impl FeedFilterConfig for FullTextConfig {
  type Filter = FullTextFilter;

  async fn build(&self) -> Result<Self::Filter> {
    let client = reqwest::Client::builder()
      .user_agent(crate::util::USER_AGENT)
      .tcp_keepalive(Some(std::time::Duration::from_secs(10)))
      .build()?;

    Ok(FullTextFilter { client })
  }
}

impl FullTextFilter {
  async fn fetch_full_post(&self, mut post: Post) -> Result<Post> {
    let resp = self.client.get(&post.link).send().await?;
    let resp = resp.error_for_status()?;
    post.description = resp.text().await?;
    Ok(post)
  }

  async fn fetch_all_posts(&self, posts: Vec<Post>) -> Result<Vec<Post>> {
    stream::iter(posts)
      .map(|post| self.fetch_full_post(post))
      .buffered(20)
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
