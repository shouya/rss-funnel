use std::sync::Arc;
use std::time::Duration;

use futures::{stream, StreamExt};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::cache::TimedLruCache;
use crate::client::{self, Client};
use crate::feed::{Feed, Post};
use crate::html::convert_relative_url;
use crate::{ConfigError, Error, Result};

use super::html::{KeepElement, KeepElementConfig};
use super::{FeedFilter, FeedFilterConfig, FilterContext};
use crate::feed::NormalizedPost;

type PostCache = TimedLruCache<NormalizedPost, Post>;

const DEFAULT_PARALLELISM: usize = 20;

#[derive(
  JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash,
)]
pub struct FullTextConfig {
  /// The maximum number of concurrent requests
  parallelism: Option<usize>,
  /// Whether to simplify the HTML before saving it
  simplify: Option<bool>,
  /// Whether to append the full text to the body or replace it
  append_mode: Option<bool>,
  /// Keep only content inside an element of the full text
  keep_element: Option<KeepElementConfig>,
  /// Whether to keep the GUID of the original post
  keep_guid: Option<bool>,
  /// The client configuration
  client: Option<client::ClientConfig>,
}

pub struct FullTextFilter {
  client: Client,
  parallelism: usize,
  append_mode: bool,
  keep_element: Option<KeepElement>,
  simplify: bool,
  keep_guid: bool,
  post_cache: PostCache,
}

#[async_trait::async_trait]
impl FeedFilterConfig for FullTextConfig {
  type Filter = FullTextFilter;

  async fn build(self) -> Result<Self::Filter, ConfigError> {
    // default cache ttl is 12 hours
    let default_cache_ttl = Duration::from_secs(12 * 60 * 60);
    let conf_client = self.client.unwrap_or_default();
    let client = conf_client.build(default_cache_ttl)?;
    let parallelism = self.parallelism.unwrap_or(DEFAULT_PARALLELISM);
    let append_mode = self.append_mode.unwrap_or(false);
    let simplify = self.simplify.unwrap_or(false);
    let keep_guid = self.keep_guid.unwrap_or(false);
    let keep_element = match self.keep_element {
      None => None,
      Some(c) => Some(c.build().await?),
    };
    let post_cache = PostCache::new(
      conf_client.get_cache_size(),
      conf_client.get_cache_ttl(default_cache_ttl),
    );

    Ok(FullTextFilter {
      simplify,
      client,
      parallelism,
      append_mode,
      keep_guid,
      keep_element,
      post_cache,
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
    let link = post.link_or_err()?.to_owned();
    let text = self.fetch_html(&link).await?;

    // Optimization: the strip_post_content can be CPU intensive. Spawn the blocking
    // task on a different CPU to improve parallelism.
    let simplify = self.simplify;
    let keep_element = Arc::new(self.keep_element.clone());
    let text = tokio::task::spawn_blocking(move || {
      strip_post_content(text, &link, simplify, keep_element)
    })
    .await?;

    post.ensure_body();
    post.modify_bodies(|body| {
      if self.append_mode {
        body.push_str("\n<br><hr><br>\n");
        body.push_str(&text);
      } else {
        body.replace_range(.., &text);
      };
    });

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
    // append the error message to the body instead of failing
    // completely.
    match self.try_fetch_full_post(&mut post).await {
      Ok(_) => Ok(post),
      Err(e) => {
        let message = format!("\n<br>\n<br>\nerror fetching full text: {}", e);
        post.modify_bodies(|body| {
          body.push_str(&message);
        });
        Ok(post)
      }
    }
  }

  async fn fetch_full_post_cached(&self, post: Post) -> Result<Post> {
    let normalized_post = post.normalize();
    if let Some(result_post) = self.post_cache.get_cached(&normalized_post) {
      return Ok(result_post);
    };

    match self.fetch_full_post(post).await {
      Ok(result_post) => {
        self.post_cache.insert(normalized_post, result_post.clone());
        Ok(result_post)
      }
      Err(e) => Err(e),
    }
  }

  async fn fetch_all_posts(&self, posts: Vec<Post>) -> Result<Vec<Post>> {
    stream::iter(posts)
      .map(|post| self.fetch_full_post_cached(post))
      .buffered(self.parallelism)
      .collect::<Vec<_>>()
      .await
      .into_iter()
      .collect()
  }
}

#[async_trait::async_trait]
impl FeedFilter for FullTextFilter {
  async fn run(
    &self,
    _ctx: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    let posts = feed.take_posts();
    let posts = self.fetch_all_posts(posts).await?;
    feed.set_posts(posts);
    Ok(feed)
  }
}

fn strip_post_content(
  html: String,
  link: &str,
  simplify: bool,
  keep_element: Arc<Option<KeepElement>>,
) -> String {
  let mut html = scraper::Html::parse_document(&html);
  convert_relative_url(&mut html, link);
  let mut text = html.html();

  if simplify {
    text = super::simplify_html::simplify(&text, link).unwrap_or(text);
  } else {
    text = crate::html::html_body(&text);
  }

  if let Some(k) = keep_element.as_ref() {
    k.filter_body(&mut text)
  }

  text
}
