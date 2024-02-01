use futures::{stream, StreamExt};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::client::{self, Client};
use crate::feed::{Feed, Post};
use crate::html::convert_relative_url;
use crate::util::{Error, Result};

use super::html::{KeepElement, KeepElementConfig};
use super::{FeedFilter, FeedFilterConfig};

const DEFAULT_PARALLELISM: usize = 20;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FullTextConfig {
  parallelism: Option<usize>,
  simplify: Option<bool>,
  append_mode: Option<bool>,
  keep_element: Option<KeepElementConfig>,
  keep_guid: Option<bool>,
  client: Option<client::ClientConfig>,
}

pub struct FullTextFilter {
  client: Client,
  parallelism: usize,
  append_mode: bool,
  keep_element: Option<KeepElement>,
  simplify: bool,
  keep_guid: bool,
}

#[async_trait::async_trait]
impl FeedFilterConfig for FullTextConfig {
  type Filter = FullTextFilter;

  async fn build(self) -> Result<Self::Filter> {
    let client = self.client.unwrap_or_default().build()?;
    let parallelism = self.parallelism.unwrap_or(DEFAULT_PARALLELISM);
    let append_mode = self.append_mode.unwrap_or(false);
    let simplify = self.simplify.unwrap_or(false);
    let keep_guid = self.keep_guid.unwrap_or(false);
    let keep_element = match self.keep_element {
      None => None,
      Some(c) => Some(c.build().await?),
    };

    Ok(FullTextFilter {
      simplify,
      client,
      parallelism,
      append_mode,
      keep_guid,
      keep_element,
    })
  }
}

impl FullTextFilter {
  async fn fetch_html(&self, url: &str) -> Result<String> {
    let url = Url::parse(url)?;
    let resp = self.client.get(&url).await?;
    let content_type = resp.content_type().unwrap_or(mime::TEXT_HTML);

    if content_type.essence_str() != "text/html" {
      return Err(Error::Message(format!(
        "unexpected content type: {}",
        content_type
      )));
    }

    let resp = resp.error_for_status()?;
    let text = resp.text()?;

    Ok(text)
  }

  async fn try_fetch_full_post(&self, post: &mut Post) -> Result<()> {
    let link = post.link_or_err()?;
    let text = self.fetch_html(link).await?;

    let mut html = scraper::Html::parse_document(&text);
    convert_relative_url(&mut html, link);
    let mut text = html.html();

    match self
      .keep_element
      .as_ref()
      .and_then(|k| k.filter_description(&text))
    {
      Some(filtered) => {
        text = filtered;
      }
      None => {
        text = format!(
          "<p>Failed to filter description with keep_element</p>\n{}",
          text
        );
      }
    }

    if self.simplify {
      text = super::simplify_html::simplify(&text, link).unwrap_or(text);
    };

    let description = post.description_or_insert();
    if self.append_mode {
      description.push_str("\n<br><hr><br>\n");
      description.push_str(&text);
    } else {
      *description = text;
    };

    if !self.keep_guid {
      if let Some(mut guid) = post.guid().map(|v| v.to_string()) {
        guid.push_str("-full");
        post.set_guid(guid);
      }
    }

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
