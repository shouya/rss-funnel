mod conversion;
mod extension;

use chrono::DateTime;
use paste::paste;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use url::Url;

use crate::html::convert_relative_url;
use crate::html::html_body;
use crate::source::FromScratch;
use crate::util::Error;
use crate::util::Result;

use extension::ExtensionExt;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum Feed {
  Rss(rss::Channel),
  Atom(atom_syndication::Feed),
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Copy,
)]
#[serde(rename_all = "lowercase")]
pub enum FeedFormat {
  /// RSS 2.0
  Rss,
  /// Atom 1.0
  Atom,
}

impl Feed {
  pub fn format(&self) -> FeedFormat {
    match self {
      Feed::Rss(_) => FeedFormat::Rss,
      Feed::Atom(_) => FeedFormat::Atom,
    }
  }

  pub fn into_format(self, format: FeedFormat) -> Self {
    use conversion::W;

    if self.format() == format {
      return self;
    }

    match (self, format) {
      (Feed::Rss(channel), FeedFormat::Atom) => {
        let feed: atom_syndication::Feed = W(channel).into();
        Feed::Atom(feed)
      }
      (Feed::Atom(feed), FeedFormat::Rss) => {
        let channel: rss::Channel = W(feed).into();
        Feed::Rss(channel)
      }
      (original_self, _) => original_self,
    }
  }

  pub fn from_rss_content(content: &[u8]) -> Result<Self> {
    let cursor = std::io::Cursor::new(content);
    let channel = rss::Channel::read_from(cursor)?;
    Ok(Feed::Rss(channel))
  }

  pub fn from_atom_content(content: &[u8]) -> Result<Self> {
    let cursor = std::io::Cursor::new(content);
    let feed = atom_syndication::Feed::read_from(cursor)?;
    Ok(Feed::Atom(feed))
  }

  pub fn from_xml_content(content: &[u8]) -> Result<Self> {
    Feed::from_rss_content(content)
      .or_else(|_| Feed::from_atom_content(content))
  }

  pub fn content_type(&self) -> &'static str {
    match self {
      Feed::Rss(_) => "application/rss+xml",
      Feed::Atom(_) => "application/atom+xml",
    }
  }

  pub fn serialize(&self, pretty: bool) -> Result<String> {
    let mut buffer = vec![];

    match self {
      Feed::Rss(channel) => {
        if pretty {
          channel.pretty_write_to(&mut buffer, b' ', 2)?;
        } else {
          channel.write_to(&mut buffer)?;
        }
      }
      Feed::Atom(feed) => {
        let mut feed = feed.clone();
        fix_escaping_in_extension_attr(&mut feed);
        let mut conf = atom_syndication::WriteConfig {
          indent_size: None,
          write_document_declaration: true,
        };

        if pretty {
          conf.indent_size = Some(2);
        }

        feed.write_with_config(&mut buffer, conf)?;
      }
    };

    let s = String::from_utf8_lossy(&buffer).into_owned();
    Ok(s)
  }

  #[allow(clippy::field_reassign_with_default)]
  pub fn from_html_content(content: &str, url: &Url) -> Result<Self> {
    let item = Post::from_html_content(content, url)?;

    let mut channel = rss::Channel::default();
    channel.title = item.title().expect("title should present").to_string();
    channel.link = url.to_string();

    let mut feed = Feed::Rss(channel);
    feed.set_posts(vec![item]);

    Ok(feed)
  }

  pub fn take_posts(&mut self) -> Vec<Post> {
    match self {
      Feed::Rss(channel) => {
        let posts = channel.items.split_off(0);
        posts.into_iter().map(Post::Rss).collect()
      }
      Feed::Atom(feed) => {
        let posts = feed.entries.split_off(0);
        posts.into_iter().map(Post::Atom).collect()
      }
    }
  }

  pub fn set_posts(&mut self, posts: Vec<Post>) {
    #[allow(clippy::unnecessary_filter_map)]
    match self {
      Feed::Rss(channel) => {
        channel.items = posts
          .into_iter()
          .filter_map(|post| match post {
            Post::Rss(item) => Some(item),
            _ => None,
          })
          .collect();
      }
      Feed::Atom(feed) => {
        feed.entries = posts
          .into_iter()
          .filter_map(|post| match post {
            Post::Atom(item) => Some(item),
            _ => None,
          })
          .collect();
      }
    }
  }

  #[allow(unused)]
  pub fn title(&self) -> &str {
    match self {
      Feed::Rss(channel) => &channel.title,
      Feed::Atom(feed) => feed.title.as_str(),
    }
  }

  pub fn merge(&mut self, other: Feed) -> Result<()> {
    match (self, other) {
      (Feed::Rss(channel), Feed::Rss(other)) => {
        channel.namespaces.extend(other.namespaces);
        channel.items.extend(other.items);
      }
      (Feed::Atom(feed), Feed::Atom(other)) => {
        feed.namespaces.extend(other.namespaces);
        feed.entries.extend(other.entries);
      }
      (Feed::Rss(_), _) => {
        return Err(Error::FeedMerge("cannot merge atom into rss"));
      }
      (Feed::Atom(_), _) => {
        return Err(Error::FeedMerge("cannot merge rss into atom"));
      }
    }

    Ok(())
  }

  // reorder the entries in a feed so that the newest ones come first
  pub fn reorder(&mut self) {
    use std::cmp::Reverse;

    match self {
      Feed::Rss(channel) => {
        channel
          .items
          .sort_unstable_by_key(|item| Reverse(rss_item_timestamp(item)));
      }
      Feed::Atom(feed) => {
        feed
          .entries
          .sort_unstable_by_key(|entry| Reverse(entry.updated));
      }
    }
  }
}

#[cfg(test)]
impl TryFrom<Feed> for rss::Channel {
  type Error = ();

  fn try_from(feed: Feed) -> Result<Self, Self::Error> {
    match feed {
      Feed::Rss(channel) => Ok(channel),
      _ => Err(()),
    }
  }
}

#[cfg(test)]
impl TryFrom<Feed> for atom_syndication::Feed {
  type Error = ();

  fn try_from(feed: Feed) -> Result<Self, Self::Error> {
    match feed {
      Feed::Atom(feed) => Ok(feed),
      _ => Err(()),
    }
  }
}

impl From<&FromScratch> for Feed {
  fn from(config: &FromScratch) -> Self {
    use FeedFormat::*;
    match config.format {
      Rss => {
        let mut channel = rss::Channel {
          title: config.title.clone(),
          ..Default::default()
        };

        if let Some(link) = &config.link {
          channel.link = link.clone();
        }
        if let Some(description) = &config.description {
          channel.description = description.clone();
        }

        Feed::Rss(channel)
      }
      Atom => {
        let mut feed = atom_syndication::Feed {
          title: atom_syndication::Text::plain(config.title.clone()),
          ..Default::default()
        };
        if let Some(link) = &config.link {
          feed.links.push(atom_syndication::Link {
            href: link.clone(),
            ..Default::default()
          });
        }
        if let Some(description) = &config.description {
          feed.subtitle =
            Some(atom_syndication::Text::plain(description.clone()));
        }

        Feed::Atom(feed)
      }
    }
  }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Post {
  Rss(rss::Item),
  Atom(atom_syndication::Entry),
}

enum PostField {
  Title,
  Link,
  Author,
  Guid,
}

impl Post {
  pub fn set_pub_date(&mut self, date: DateTime<chrono::FixedOffset>) {
    match self {
      Post::Rss(item) => {
        item.pub_date = Some(date.to_rfc2822());
      }
      Post::Atom(item) => {
        item.updated = date;
      }
    }
  }

  #[allow(unused)]
  pub fn pub_date(&self) -> Option<DateTime<chrono::FixedOffset>> {
    match self {
      Post::Rss(item) => item
        .pub_date
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc2822(s).ok()),
      Post::Atom(item) => Some(item.updated),
    }
  }

  // the order should match the actual display order in rss
  // readers. This allows ensure_body to return the body field that is
  // most likely to affect the actual appearance.
  #[allow(clippy::option_map_unit_fn)]
  pub fn bodies_mut(&mut self) -> Vec<&mut String> {
    let mut bodies = Vec::new();
    match self {
      Post::Rss(item) => {
        item.content.as_mut().map(|v| bodies.push(v));
        item.description.as_mut().map(|v| bodies.push(v));
        item
          .extensions
          .tags_mut_with_names(&["media:description"])
          .into_iter()
          .filter_map(|tag| tag.value.as_mut())
          .for_each(|v| bodies.push(v));
      }
      Post::Atom(item) => {
        item
          .content
          .as_mut()
          .and_then(|c| c.value.as_mut())
          .map(|v| bodies.push(v));
        item.summary.as_mut().map(|s| bodies.push(&mut s.value));
        item
          .extensions
          .tags_mut_with_names(&["media:description"])
          .into_iter()
          .filter_map(|tag| tag.value.as_mut())
          .for_each(|v| bodies.push(v));
      }
    }
    bodies
  }

  // Please make sure this function matches the order of bodies_mut
  #[allow(clippy::option_map_unit_fn)]
  pub fn bodies(&self) -> Vec<&str> {
    let mut bodies = Vec::new();
    match self {
      Post::Rss(item) => {
        item.content.as_deref().map(|v| bodies.push(v));
        item.description.as_deref().map(|v| bodies.push(v));
        item
          .extensions
          .tags_with_names(&["media:description"])
          .into_iter()
          .filter_map(|tag| tag.value.as_deref())
          .for_each(|v| bodies.push(v));
      }
      Post::Atom(item) => {
        item
          .content
          .as_ref()
          .and_then(|c| c.value.as_deref())
          .map(|v| bodies.push(v));
        item.summary.as_ref().map(|s| bodies.push(&s.value));
        item
          .extensions
          .tags_with_names(&["media:description"])
          .into_iter()
          .filter_map(|tag| tag.value.as_deref())
          .for_each(|v| bodies.push(v));
      }
    }
    bodies
  }

  pub fn modify_body(&mut self, mut f: impl FnMut(&mut String)) {
    for body in self.bodies_mut() {
      f(body);
    }
  }

  pub fn first_body(&self) -> Option<&str> {
    self.bodies().into_iter().next()
  }

  pub fn first_body_mut(&mut self) -> Option<&mut String> {
    self.bodies_mut().into_iter().next()
  }

  pub fn create_body(&mut self) -> &mut String {
    match self {
      Post::Rss(item) => {
        item.description = Some(String::new());
        item.description.as_mut().unwrap()
      }
      Post::Atom(item) => {
        item.summary = Some(atom_syndication::Text::html(String::new()));
        &mut item.summary.as_mut().unwrap().value
      }
    }
  }

  pub fn ensure_body(&mut self) -> &mut String {
    let needs_body = self.first_body_mut().is_none();

    if needs_body {
      self.create_body()
    } else {
      self.first_body_mut().unwrap()
    }
  }
}

impl Post {
  fn get_field(&self, field: PostField) -> Option<&str> {
    match (self, field) {
      (Post::Rss(item), PostField::Title) => item.title.as_deref(),
      (Post::Rss(item), PostField::Link) => item.link.as_deref(),
      (Post::Rss(item), PostField::Author) => item.author.as_deref(),
      (Post::Rss(item), PostField::Guid) => {
        item.guid.as_ref().map(|v| v.value.as_str())
      }
      (Post::Atom(item), PostField::Title) => Some(&item.title.value),
      (Post::Atom(item), PostField::Link) => {
        item.links.first().map(|v| v.href.as_str())
      }
      (Post::Atom(item), PostField::Author) => {
        item.authors.first().map(|v| v.name.as_str())
      }
      (Post::Atom(item), PostField::Guid) => Some(&item.id),
    }
  }

  fn set_field(&mut self, field: PostField, value: impl Into<String>) {
    match (self, field) {
      (Post::Rss(item), PostField::Title) => item.title = Some(value.into()),
      (Post::Rss(item), PostField::Link) => item.link = Some(value.into()),
      (Post::Rss(item), PostField::Author) => item.author = Some(value.into()),
      (Post::Rss(item), PostField::Guid) => {
        item.guid = Some(rss::Guid {
          value: value.into(),
          ..Default::default()
        })
      }
      (Post::Atom(item), PostField::Title) => item.title.value = value.into(),
      (Post::Atom(item), PostField::Link) => match item.links.get_mut(0) {
        Some(link) => link.href = value.into(),
        None => {
          item.links.push(atom_syndication::Link {
            href: value.into(),
            ..Default::default()
          });
        }
      },
      (Post::Atom(item), PostField::Author) => match item.authors.get_mut(0) {
        Some(author) => author.name = value.into(),
        None => {
          item.authors.push(atom_syndication::Person {
            name: value.into(),
            ..Default::default()
          });
        }
      },
      (Post::Atom(item), PostField::Guid) => item.id = value.into(),
    }
  }

  fn get_field_mut(&mut self, field: PostField) -> Option<&mut String> {
    match (self, field) {
      (Post::Rss(item), PostField::Title) => item.title.as_mut(),
      (Post::Rss(item), PostField::Link) => item.link.as_mut(),
      (Post::Rss(item), PostField::Author) => item.author.as_mut(),
      (Post::Rss(item), PostField::Guid) => {
        item.guid.as_mut().map(|v| &mut v.value)
      }
      (Post::Atom(item), PostField::Title) => Some(&mut item.title.value),
      (Post::Atom(item), PostField::Link) => {
        item.links.get_mut(0).map(|v| &mut v.href)
      }
      (Post::Atom(item), PostField::Author) => {
        item.authors.get_mut(0).map(|v| &mut v.name)
      }
      (Post::Atom(item), PostField::Guid) => Some(&mut item.id),
    }
  }

  fn get_field_mut_or_insert(&mut self, field: PostField) -> &mut String {
    match (self, field) {
      (Post::Rss(item), PostField::Title) => {
        item.title.get_or_insert_with(String::new)
      }
      (Post::Rss(item), PostField::Link) => {
        item.link.get_or_insert_with(String::new)
      }
      (Post::Rss(item), PostField::Author) => {
        item.author.get_or_insert_with(String::new)
      }
      (Post::Rss(item), PostField::Guid) => {
        &mut item
          .guid
          .get_or_insert_with(|| rss::Guid {
            value: String::new(),
            ..Default::default()
          })
          .value
      }
      (Post::Atom(item), PostField::Title) => &mut item.title.value,
      (Post::Atom(item), PostField::Link) => {
        &mut vec_first_or_insert(
          &mut item.links,
          atom_syndication::Link {
            href: String::new(),
            ..Default::default()
          },
        )
        .href
      }
      (Post::Atom(item), PostField::Author) => {
        &mut vec_first_or_insert(
          &mut item.authors,
          atom_syndication::Person {
            name: String::new(),
            ..Default::default()
          },
        )
        .name
      }
      (Post::Atom(item), PostField::Guid) => &mut item.id,
    }
  }
}

macro_rules! impl_post_accessors {
  ($($key:ident => $field:ident);*) => {
    paste! {
      impl Post {
        $(
        #[allow(unused)]
        pub fn $key(&self) -> Option<&str> {
          self.get_field(PostField::$field)
        }

        #[allow(unused)]
        pub fn [<set_ $key>](&mut self, value: impl Into<String>) {
          self.set_field(PostField::$field, value);
        }

        #[allow(unused)]
        pub fn [<$key _mut>](&mut self) -> Option<&mut String> {
          self.get_field_mut(PostField::$field)
        }

        #[allow(unused)]
        pub fn [<$key _or_err>](&self) -> Result<&str> {
          match self.$key() {
            Some(value) => Ok(value),
            None => Err(Error::FeedParse(concat!("missing ", stringify!($key)))),
          }
        }

        #[allow(unused)]
        pub fn [<$key _or_insert>](&mut self) -> &mut String {
          self.get_field_mut_or_insert(PostField::$field)
        }
        )*
      }
    }
  };
}

impl_post_accessors! {
  title => Title;
  link => Link;
  author => Author;
  guid => Guid
}

impl Post {
  #[allow(clippy::field_reassign_with_default)]
  fn from_html_content(content: &str, url: &Url) -> Result<Self> {
    // convert any relative urls to absolute urls
    let mut html = scraper::Html::parse_document(content);
    convert_relative_url(&mut html, url.as_str());
    let content = html.html();
    let mut reader = std::io::Cursor::new(&content);
    let product = readability::extractor::extract(&mut reader, url)?;

    let content_body = html_body(&content);
    let mut item = rss::Item::default();
    item.title = Some(product.title);
    item.description = Some(content_body);
    item.link = Some(url.to_string());
    item.guid = Some(rss::Guid {
      value: url.to_string(),
      ..Default::default()
    });
    Ok(Post::Rss(item))
  }
}

fn vec_first_or_insert<T>(v: &mut Vec<T>, def: T) -> &mut T {
  if !v.is_empty() {
    return v.first_mut().unwrap();
  }

  v.push(def);
  v.first_mut().unwrap()
}

fn fix_escaping_in_extension_attr(feed: &mut atom_syndication::Feed) {
  // atom_syndication unescapes the html entities in the extension attributes, but it doesn't
  // escape them back when serializing the feed, so we need to do it ourselves
  for entry in &mut feed.entries {
    for (_ns, elems) in entry.extensions.iter_mut() {
      for (_ns2, exts) in elems.iter_mut() {
        for ext in exts {
          if let Some(url) = ext.attrs.get_mut("url") {
            *url = url.replace('&', "&amp;");
          }
        }
      }
    }
  }
}

fn rss_item_timestamp(item: &rss::Item) -> Option<i64> {
  use chrono::FixedOffset;

  let pub_date = item.pub_date.as_ref()?;

  let Ok(date) = DateTime::<FixedOffset>::parse_from_rfc2822(pub_date) else {
    return None;
  };

  Some(date.timestamp())
}

impl axum::response::IntoResponse for Feed {
  fn into_response(self) -> axum::response::Response {
    let content = self.serialize(true).expect("failed serializing feed");
    let content_type = self.content_type();
    let headers = [("content-type", content_type)];

    (http::StatusCode::OK, headers, content).into_response()
  }
}
