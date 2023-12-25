use std::time::Duration;

use duration_str::deserialize_duration;
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::feed::{Feed, Post};
use crate::html::convert_relative_url;
use crate::util::Result;

use super::{FeedFilter, FeedFilterConfig};

const DEFAULT_PARALLELISM: usize = 20;

#[derive(Serialize, Deserialize)]
pub struct FullTextConfig {
  #[serde(default = "default_timeout")]
  #[serde(deserialize_with = "deserialize_duration")]
  timeout: Duration,
  parallelism: Option<usize>,
  simplify: Option<bool>,
  append_mode: Option<bool>,
}

pub struct FullTextFilter {
  client: reqwest::Client,
  parallelism: usize,
  append_mode: bool,
  simplify: bool,
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
    let append_mode = self.append_mode.unwrap_or(false);
    let simplify = self.simplify.unwrap_or(false);

    Ok(FullTextFilter {
      simplify,
      client,
      parallelism,
      append_mode,
    })
  }
}

impl FullTextFilter {
  async fn try_fetch_full_post(&self, post: &mut Post) -> Result<()> {
    let link = post.link_or_err()?;
    let resp = self.client.get(link).send().await?;
    let resp = resp.error_for_status()?;
    let text = resp.text().await?;

    let mut html = scraper::Html::parse_document(&text);
    convert_relative_url(&mut html, link);
    let text = html.html();

    let text = if self.simplify {
      super::simplify_html::simplify(&text, link).unwrap_or(text)
    } else {
      text
    };

    let description = post.description_or_insert();
    if self.append_mode {
      description.push_str("\n<br><hr><br>\n");
      description.push_str(&text);
    } else {
      *description = text;
    };
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
        post.description_or_insert().push_str(&message);
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
    let posts = feed.take_posts();
    let posts = self.fetch_all_posts(posts).await?;
    feed.set_posts(posts);
    Ok(())
  }
}

fn default_timeout() -> Duration {
  Duration::from_secs(10)
}
