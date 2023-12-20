use std::borrow::Cow;
use std::collections::HashMap;

use axum::response::IntoResponse;
use http::StatusCode;
use serde::Deserialize;
use serde::Serialize;

use crate::util::Error;
use crate::util::Result;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Feed {
  pub title: String,
  pub link: String,
  pub description: String,
  pub extra: HashMap<String, String>,
  pub posts: Vec<Post>,
}

impl Feed {
  pub fn from_rss_content(bytes: &[u8]) -> Result<Self> {
    let channel = rss::Channel::read_from(bytes)?;
    let feed = Self::try_from(channel)?;
    Ok(feed)
  }

  pub fn into_resp(self) -> Result<impl IntoResponse> {
    let headers = [(http::header::CONTENT_TYPE, "application/rss+xml")];
    let body = rss::Channel::from(self).to_string();

    Ok((StatusCode::OK, headers, body))
  }
}

impl TryFrom<rss::Channel> for Feed {
  type Error = Error;
  fn try_from(channel: rss::Channel) -> Result<Self> {
    let title = channel.title;
    let link = channel.link;
    let description = channel.description;
    let extra = HashMap::new();

    let posts = channel
      .items
      .into_iter()
      .map(Post::try_from)
      .collect::<Result<Vec<_>>>()?;

    Ok(Self {
      title,
      link,
      description,
      extra,
      posts,
    })
  }
}

impl From<Feed> for rss::Channel {
  fn from(feed: Feed) -> Self {
    let title = feed.title;
    let link = feed.link;
    let description = feed.description;

    let items = feed.posts.into_iter().map(rss::Item::from).collect();

    Self {
      title,
      link,
      description,
      items,
      ..Default::default()
    }
  }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Post {
  pub guid: String,
  pub title: String,
  pub description: String,
  pub authors: Vec<String>,
  pub link: String,
  pub extra: HashMap<String, String>,
  pub pub_date: Option<String>,
}

impl TryFrom<rss::Item> for Post {
  type Error = Error;

  fn try_from(item: rss::Item) -> Result<Self> {
    let link = item.link.ok_or_else(|| Error::FeedParse("link in item"))?;
    let guid = item
      .guid
      .map(|guid| guid.value)
      .unwrap_or_else(|| link.clone());

    let title = item
      .title
      .ok_or_else(|| Error::FeedParse("title in item"))?;

    let description = item
      .description
      .ok_or_else(|| Error::FeedParse("description in item"))?;

    let authors = item.author.into_iter().collect();

    let pub_date = item.pub_date;
    let extra = HashMap::new();

    Ok(Self {
      guid,
      title,
      description,
      authors,
      link,
      extra,
      pub_date,
    })
  }
}

impl From<Post> for rss::Item {
  fn from(post: Post) -> Self {
    let guid = Some(rss::Guid {
      value: post.guid,
      ..Default::default()
    });
    let title = Some(post.title);
    let description = Some(post.description);
    let author = Some(post.authors.join(","));
    let link = Some(post.link);
    let pub_date = post.pub_date;

    Self {
      guid,
      title,
      description,
      author,
      link,
      pub_date,
      ..Default::default()
    }
  }
}

impl Post {
  pub fn get_field(&self, field: &str) -> Option<Cow<str>> {
    match field {
      "guid" => Some(Cow::from(&self.guid)),
      "title" => Some(Cow::from(&self.title)),
      "description" => Some(Cow::from(&self.description)),
      "link" => Some(Cow::from(&self.link)),
      "pub_date" => self.pub_date.as_ref().map(Cow::from),
      _ => self.extra.get(field).map(Cow::from),
    }
  }
}
