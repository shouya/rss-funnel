use std::collections::HashSet;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::client::{Client, ClientConfig};
use crate::feed::{Feed, Post};
use crate::filter::{FeedFilter, FeedFilterConfig, FilterContext};
use crate::{ConfigError, Error, Result};

const DEFAULT_CACHE_TTL_SECS: u64 = 5 * 60;

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct JsonToFeedConfig {
  /// Optional explicit URL to fetch JSON from. Falls back to the endpoint source URL.
  #[serde(default)]
  pub url: Option<String>,
  /// JSONPath that selects the collection of items to turn into posts.
  pub items: String,
  /// Field mapping for each JSON item.
  #[serde(default)]
  pub map: FieldMap,
  /// Optional feed metadata mapping sourced from the same JSON document.
  #[serde(default)]
  pub feed: FeedMap,
  /// Extra chrono date formats to try in addition to RFC 3339 and RFC 2822.
  #[serde(default)]
  pub date_formats: Vec<String>,
  /// Optional HTTP client configuration for fetching the JSON source.
  #[serde(default)]
  pub client: Option<ClientConfig>,
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, Hash,
)]
#[serde(default)]
pub struct FieldMap {
  pub title: Option<String>,
  pub link: Option<String>,
  pub guid: Option<String>,
  pub description: Option<String>,
  pub content_html: Option<String>,
  pub author: Option<String>,
  pub categories: Option<String>,
  pub pub_date: Option<DateField>,
  pub enclosure_url: Option<String>,
  pub enclosure_type: Option<String>,
  pub enclosure_length: Option<String>,
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, Hash,
)]
#[serde(default)]
pub struct FeedMap {
  pub title: Option<String>,
  pub link: Option<String>,
  pub description: Option<String>,
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct DateField {
  pub path: String,
  #[serde(default)]
  pub parse: Vec<String>,
}

pub struct JsonToFeedFilter {
  url: Option<Url>,
  items_path: String,
  map: FieldMap,
  feed_map: FeedMap,
  date_formats: Vec<String>,
  client: Client,
}

#[async_trait::async_trait]
impl FeedFilterConfig for JsonToFeedConfig {
  type Filter = JsonToFeedFilter;

  async fn build(self) -> Result<Self::Filter, ConfigError> {
    let JsonToFeedConfig {
      url,
      items,
      map,
      feed,
      date_formats,
      client,
    } = self;

    let parsed_url = url.map(|u| Url::parse(&u)).transpose()?;
    let client_cfg = client.unwrap_or_default();
    let client =
      client_cfg.build(Duration::from_secs(DEFAULT_CACHE_TTL_SECS))?;

    Ok(JsonToFeedFilter {
      url: parsed_url,
      items_path: items,
      map,
      feed_map: feed,
      date_formats,
      client,
    })
  }
}

#[async_trait::async_trait]
impl FeedFilter for JsonToFeedFilter {
  async fn run(&self, ctx: &mut FilterContext, mut feed: Feed) -> Result<Feed> {
    let url = if let Some(url) = &self.url {
      url.clone()
    } else if let Some(source) = ctx.source() {
      source.clone()
    } else {
      return Err(Error::Message(
        "json_to_feed: the filter needs a URL (set `url` or provide a source \
         URL)"
          .into(),
      ));
    };

    let response = self.client.get(&url).await?.error_for_status()?;
    let body = response.text()?;
    let root: Value = serde_json::from_str(&body).map_err(|err| {
      Error::Message(format!("json_to_feed: JSON parse error: {err}"))
    })?;

    apply_feed_metadata(&mut feed, &self.feed_map, &root)?;

    let items =
      jsonpath_lib::select(&root, &self.items_path).map_err(|err| {
        Error::Message(format!(
          "json_to_feed: failed to apply items JSONPath `{}`: {err}",
          self.items_path
        ))
      })?;

    let mut posts = Vec::with_capacity(items.len());
    let mut seen_guids = HashSet::new();

    for item in items {
      let title = select_string(item, self.map.title.as_deref())?;
      let link = select_string(item, self.map.link.as_deref())?;
      let guid = select_string(item, self.map.guid.as_deref())?;
      let description = select_string(item, self.map.description.as_deref())?;
      let content_html = select_string(item, self.map.content_html.as_deref())?;
      let author = select_string(item, self.map.author.as_deref())?;
      let categories = select_strings(item, self.map.categories.as_deref())?;
      let enclosure_url =
        select_string(item, self.map.enclosure_url.as_deref())?;
      let enclosure_type =
        select_string(item, self.map.enclosure_type.as_deref())?;
      let enclosure_length =
        select_string(item, self.map.enclosure_length.as_deref())?;
      let published =
        parse_published(item, self.map.pub_date.as_ref(), &self.date_formats)?;

      let mut guid = guid
        .or_else(|| link.clone())
        .unwrap_or_else(|| make_guid(item));
      ensure_unique_guid(&mut guid, &mut seen_guids);

      posts.push(match &feed {
        Feed::Rss(_) => Post::Rss(build_rss_item(
          title.clone(),
          link.clone(),
          guid.clone(),
          description.clone(),
          content_html.clone(),
          author.clone(),
          categories.clone(),
          published,
          enclosure_url.clone(),
          enclosure_type.clone(),
          enclosure_length.clone(),
        )),
        Feed::Atom(_) => Post::Atom(build_atom_entry(
          title.clone(),
          link.clone(),
          guid.clone(),
          description.clone(),
          content_html.clone(),
          author.clone(),
          categories.clone(),
          published,
        )),
      });
    }

    feed.set_posts(posts);
    Ok(feed)
  }
}

fn apply_feed_metadata(
  feed: &mut Feed,
  map: &FeedMap,
  root: &Value,
) -> Result<(), Error> {
  if let Some(title_path) = map.title.as_deref() {
    if let Some(title) = select_string(root, Some(title_path))? {
      match feed {
        Feed::Rss(channel) => channel.title = title,
        Feed::Atom(atom) => atom.title = atom_syndication::Text::plain(title),
      }
    }
  }

  if let Some(link_path) = map.link.as_deref() {
    if let Some(link) = select_string(root, Some(link_path))? {
      match feed {
        Feed::Rss(channel) => channel.link = link,
        Feed::Atom(atom) => {
          if let Some(first) = atom.links.first_mut() {
            first.href = link.clone();
          } else {
            atom.links.push(atom_syndication::Link {
              href: link,
              ..Default::default()
            });
          }
        }
      }
    }
  }

  if let Some(desc_path) = map.description.as_deref() {
    if let Some(description) = select_string(root, Some(desc_path))? {
      match feed {
        Feed::Rss(channel) => channel.description = description,
        Feed::Atom(atom) => {
          atom.subtitle = Some(atom_syndication::Text::plain(description));
        }
      }
    }
  }

  Ok(())
}

fn select_string(
  root: &Value,
  path: Option<&str>,
) -> Result<Option<String>, Error> {
  let Some(path) = path else { return Ok(None) };
  let values = jsonpath_lib::select(root, path).map_err(|err| {
    Error::Message(format!("json_to_feed: JSONPath `{path}` failed: {err}"))
  })?;
  Ok(values.into_iter().find_map(value_to_string))
}

fn select_strings(
  root: &Value,
  path: Option<&str>,
) -> Result<Vec<String>, Error> {
  let Some(path) = path else { return Ok(vec![]) };
  let values = jsonpath_lib::select(root, path).map_err(|err| {
    Error::Message(format!("json_to_feed: JSONPath `{path}` failed: {err}"))
  })?;
  Ok(
    values
      .into_iter()
      .filter_map(value_to_string)
      .filter(|value| !value.is_empty())
      .collect(),
  )
}

fn value_to_string(value: &Value) -> Option<String> {
  match value {
    Value::String(s) => {
      let trimmed = s.trim();
      if trimmed.is_empty() {
        None
      } else {
        Some(trimmed.to_owned())
      }
    }
    Value::Number(n) => Some(n.to_string()),
    Value::Bool(b) => Some(b.to_string()),
    _ => None,
  }
}

fn parse_published(
  item: &Value,
  field: Option<&DateField>,
  global_formats: &[String],
) -> Result<Option<DateTime<FixedOffset>>, Error> {
  let Some(field) = field else { return Ok(None) };
  let Some(raw_value) = select_string(item, Some(&field.path))? else {
    return Ok(None);
  };

  if raw_value.is_empty() {
    return Ok(None);
  }

  if let Ok(parsed) = DateTime::parse_from_rfc3339(&raw_value) {
    return Ok(Some(parsed));
  }
  if let Ok(parsed) = DateTime::parse_from_rfc2822(&raw_value) {
    return Ok(Some(parsed));
  }

  for fmt in field.parse.iter().chain(global_formats.iter()) {
    if let Ok(parsed) = DateTime::parse_from_str(&raw_value, fmt) {
      return Ok(Some(parsed));
    }

    if let Ok(parsed) = NaiveDateTime::parse_from_str(&raw_value, fmt) {
      let offset = FixedOffset::east_opt(0).unwrap();
      return Ok(Some(offset.from_local_datetime(&parsed).unwrap()));
    }
  }

  Ok(None)
}

fn make_guid(item: &Value) -> String {
  let json = serde_json::to_vec(item).unwrap_or_default();
  blake3::hash(&json).to_hex().to_string()
}

fn ensure_unique_guid(guid: &mut String, seen: &mut HashSet<String>) {
  if seen.insert(guid.clone()) {
    return;
  }

  let mut counter = 1usize;
  loop {
    let candidate = format!("{guid}#{counter}");
    if seen.insert(candidate.clone()) {
      *guid = candidate;
      break;
    }
    counter += 1;
  }
}

fn build_rss_item(
  title: Option<String>,
  link: Option<String>,
  guid: String,
  description: Option<String>,
  content_html: Option<String>,
  author: Option<String>,
  categories: Vec<String>,
  published: Option<DateTime<FixedOffset>>,
  enclosure_url: Option<String>,
  enclosure_type: Option<String>,
  enclosure_length: Option<String>,
) -> rss::Item {
  let mut item = rss::Item::default();
  item.set_title(title);
  item.set_link(link.clone());
  item.set_author(author);
  item.set_description(description);
  item.content = content_html;
  if let Some(date) = published {
    item.set_pub_date(date.to_rfc2822());
  }

  if let Some(link) = link {
    let permalink = link == guid;
    item.set_guid(Some(rss::Guid {
      value: guid,
      permalink,
    }));
  } else {
    item.set_guid(Some(rss::Guid {
      value: guid,
      permalink: false,
    }));
  }

  if !categories.is_empty() {
    item.categories = categories
      .into_iter()
      .map(|name| {
        let mut category = rss::Category::default();
        category.set_name(name);
        category
      })
      .collect();
  }

  if let Some(url) = enclosure_url {
    let mime_type =
      enclosure_type.unwrap_or_else(|| "application/octet-stream".into());
    let length = enclosure_length.unwrap_or_else(|| "0".into());
    item.enclosure = Some(rss::Enclosure {
      url,
      mime_type,
      length,
    });
  }

  item
}

fn build_atom_entry(
  title: Option<String>,
  link: Option<String>,
  guid: String,
  description: Option<String>,
  content_html: Option<String>,
  author: Option<String>,
  categories: Vec<String>,
  published: Option<DateTime<FixedOffset>>,
) -> atom_syndication::Entry {
  let mut entry = atom_syndication::Entry::default();
  entry.set_title(atom_syndication::Text::html(title.unwrap_or_default()));
  entry.set_id(guid);

  if let Some(link) = link {
    entry.set_links(vec![atom_syndication::Link {
      href: link,
      ..Default::default()
    }]);
  }

  let published = published.unwrap_or_else(|| Utc::now().fixed_offset());
  entry.set_updated(published);
  entry.set_published(Some(published));

  if let Some(description) = description {
    entry.set_summary(Some(atom_syndication::Text::html(description)));
  }

  if let Some(content) = content_html {
    let mut atom_content = atom_syndication::Content::default();
    atom_content.set_value(Some(content));
    atom_content.set_content_type(Some("html".into()));
    entry.set_content(Some(atom_content));
  }

  if let Some(author) = author {
    entry.set_authors(vec![atom_syndication::Person {
      name: author,
      ..Default::default()
    }]);
  }

  if !categories.is_empty() {
    entry.set_categories(
      categories
        .into_iter()
        .map(|term| {
          let mut category = atom_syndication::Category::default();
          category.set_term(term);
          category
        })
        .collect::<Vec<_>>(),
    );
  }

  entry
}

#[cfg(test)]
mod tests {
  use super::*;

  use crate::feed::FeedFormat;
  use crate::source::FromScratch;

  #[tokio::test]
  async fn maps_fixture_into_feed() {
    let yaml = r#"
url: "fixture:///json/news.json?content_type=application/json"
items: "$.items[*]"
feed:
  title: "$.meta.title"
  link: "$.meta.home"
  description: "$.meta.description"
map:
  title: "$.title"
  link: "$.url"
  guid: "$.id"
  description: "$.summary"
  content_html: "$.html"
  author: "$.author.name"
  categories: "$.tags[*]"
  pub_date:
    path: "$.published_at"
  enclosure_url: "$.enclosure.url"
  enclosure_type: "$.enclosure.type"
"#;

    let config: JsonToFeedConfig = serde_yaml::from_str(yaml).unwrap();
    let filter = config.build().await.unwrap();

    let feed = Feed::from(&FromScratch {
      format: FeedFormat::Rss,
      title: "JSON feed".into(),
      link: Some("https://example.com".into()),
      description: Some("placeholder".into()),
    });

    let mut ctx = FilterContext::new();
    let feed = filter.run(&mut ctx, feed).await.unwrap();

    let channel: rss::Channel = feed.try_into().unwrap();

    assert_eq!(channel.title(), "Example News");
    assert_eq!(channel.link(), "https://example.com");
    assert_eq!(channel.items().len(), 2);

    let first = &channel.items()[0];
    assert_eq!(first.title(), Some("Hello World"));
    assert_eq!(first.link(), Some("https://example.com/hello"));
    assert_eq!(first.guid().map(|g| g.value()), Some("101"));
    assert_eq!(first.description(), Some("Short blurb"));
    assert_eq!(
      first.content.as_deref(),
      Some("<p>Full <b>HTML</b> content</p>")
    );
    assert_eq!(first.author(), Some("Alice"));
    assert_eq!(first.categories.len(), 2);
    assert_eq!(first.categories[0].name(), "intro");
    assert_eq!(first.categories[1].name(), "general");
    assert!(first.pub_date().is_some());
    assert_eq!(
      first.enclosure.as_ref().map(|e| e.mime_type()),
      Some("audio/mpeg")
    );
  }
}
