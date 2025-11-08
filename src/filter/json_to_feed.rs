use std::time::Duration;

use jsonpath_lib::{Compiled as CompiledJsonPath, JsonPathError};
use perfect_derive::perfect_derive;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::client::{Client, ClientConfig};
use crate::feed::{Feed, FeedFormat, Post};
use crate::filter::{FeedFilter, FeedFilterConfig, FilterContext};
use crate::source::Source;
use crate::{ConfigError, Error, Result, util};

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
  pub map: ConfigFieldMap,
  /// Optional feed metadata mapping sourced from the same JSON document.
  #[serde(default)]
  pub feed: ConfigFeedMetaMap,
  /// Optional HTTP client configuration for fetching the JSON source.
  #[serde(default)]
  pub client: Option<ClientConfig>,
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(default)]
#[perfect_derive(Default)]
pub struct FieldMap<T> {
  pub title: Option<T>,
  pub link: Option<T>,
  pub guid: Option<T>,
  pub description: Option<T>,
  pub content_html: Option<T>,
  pub author: Option<T>,
  pub categories: Option<T>,
  pub pub_date: Option<T>,
  pub enclosure_url: Option<T>,
  pub enclosure_type: Option<T>,
  pub enclosure_length: Option<T>,
}

#[derive(Clone, Debug)]
enum ParsedField {
  Const(String),
  JsonPath(CompiledJsonPath),
}

#[derive(Clone, Debug)]
enum FieldValue<'a> {
  Const(String),
  Single(&'a serde_json::Value),
  Multi(Vec<&'a serde_json::Value>),
}

pub type ConfigFieldMap = FieldMap<String>;
type ParsedFieldMap = FieldMap<ParsedField>;
type FieldValues<'a> = FieldMap<FieldValue<'a>>;

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(default)]
#[perfect_derive(Default)]
pub struct FeedMetaMap<T> {
  pub title: Option<T>,
  pub link: Option<T>,
  pub description: Option<T>,
}

type ConfigFeedMetaMap = FeedMetaMap<String>;
type ParsedFeedMetaMap = FeedMetaMap<ParsedField>;
type FeedMetaValues<'a> = FeedMetaMap<FieldValue<'a>>;

pub struct JsonToFeedFilter {
  source: Source,
  items_path: CompiledJsonPath,
  item_map: ParsedFieldMap,
  feed_meta_map: ParsedFeedMetaMap,
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
      client,
    } = self;

    let source = match url.map(|u| Url::parse(&u)).transpose()? {
      Some(url) => Source::AbsoluteUrl(url),
      None => Source::Dynamic,
    };
    let client_cfg = client.unwrap_or_default();
    let client =
      client_cfg.build(Duration::from_secs(DEFAULT_CACHE_TTL_SECS))?;

    let items_path =
      CompiledJsonPath::compile(&items).map_err(ConfigError::Message)?;
    let item_map = map.parse()?;
    let feed_meta_map = feed.parse()?;

    Ok(JsonToFeedFilter {
      source,
      items_path,
      item_map,
      feed_meta_map,
      client,
    })
  }
}

#[async_trait::async_trait]
impl FeedFilter for JsonToFeedFilter {
  async fn run(&self, ctx: &mut FilterContext, mut feed: Feed) -> Result<Feed> {
    let url = self
      .source
      .full_url(ctx)
      .ok_or(Error::DynamicSourceUnspecified)?;
    let response = self.client.get(&url).await?.error_for_status()?;
    let body = response.text()?;
    let root: Value =
      serde_json::from_str(&body).map_err(Error::JsonDeserialization)?;

    let feed_meta = self.feed_meta_map.select(&root)?;
    feed_meta.apply(&mut feed)?;

    let items = self.items_path.select(&root).map_err(Error::JsonPath)?;
    let mut posts = Vec::with_capacity(items.len());

    for item in items {
      let fields = self.item_map.select(item)?;
      let post = match &feed {
        Feed::Rss(_) => fields.build_rss_post()?,
        Feed::Atom(_) => fields.build_atom_post()?,
      };

      posts.push(post);
    }

    feed.set_posts(posts);
    Ok(feed)
  }
}

impl ConfigFieldMap {
  fn parse(self) -> Result<ParsedFieldMap, ConfigError> {
    let field_map = ParsedFieldMap {
      title: self.title.map(parse_field).transpose()?,
      link: self.link.map(parse_field).transpose()?,
      guid: self.guid.map(parse_field).transpose()?,
      description: self.description.map(parse_field).transpose()?,
      content_html: self.content_html.map(parse_field).transpose()?,
      author: self.author.map(parse_field).transpose()?,
      categories: self.categories.map(parse_field).transpose()?,
      pub_date: self.pub_date.map(parse_field).transpose()?,
      enclosure_url: self.enclosure_url.map(parse_field).transpose()?,
      enclosure_type: self.enclosure_type.map(parse_field).transpose()?,
      enclosure_length: self.enclosure_length.map(parse_field).transpose()?,
    };

    Ok(field_map)
  }
}

impl ParsedFieldMap {
  fn select<'a>(&'a self, root: &'a Value) -> Result<FieldValues<'a>, Error> {
    // TODO: add context to the errors
    let field_values = FieldValues {
      title: select_field(root, &self.title, false)?,
      link: select_field(root, &self.link, false)?,
      guid: select_field(root, &self.guid, true)?,
      description: select_field(root, &self.description, true)?,
      content_html: select_field(root, &self.content_html, true)?,
      author: select_field(root, &self.author, true)?,
      categories: select_field(root, &self.categories, true)?,
      pub_date: select_field(root, &self.pub_date, true)?,
      enclosure_url: select_field(root, &self.enclosure_url, true)?,
      enclosure_type: select_field(root, &self.enclosure_type, true)?,
      enclosure_length: select_field(root, &self.enclosure_length, true)?,
    };

    Ok(field_values)
  }
}

impl FieldValue<'_> {
  fn to_string(&self) -> Result<String> {
    match self {
      FieldValue::Const(s) => Ok(s.clone()),

      FieldValue::Single(Value::String(s)) => Ok(s.trim().to_string()),
      FieldValue::Single(Value::Number(n)) => Ok(n.to_string()),
      FieldValue::Single(Value::Bool(b)) => Ok(b.to_string()),

      FieldValue::Single(other) => Err(Error::InvalidField {
        value_repr: other.to_string(),
        expected: "string",
      }),

      FieldValue::Multi(_) => Err(Error::InvalidField {
        value_repr: "(multiple values selected)".to_owned(),
        expected: "single string",
      }),
    }
  }

  fn to_strings(&self) -> Result<Vec<String>> {
    match self {
      FieldValue::Const(s) => Ok(vec![s.clone()]),
      FieldValue::Multi(values) => Ok(
        values
          .iter()
          .map(|v| FieldValue::Single(v).to_string())
          .collect::<Result<Vec<_>>>()?,
      ),
      FieldValue::Single(Value::Array(arr)) => Ok(
        arr
          .iter()
          .map(|v| FieldValue::Single(v).to_string())
          .collect::<Result<Vec<_>>>()?,
      ),
      FieldValue::Single(other) => {
        Ok(FieldValue::Single(other).to_string().map(|s| vec![s])?)
      }
    }
  }
}

macro_rules! get_field {
  (required; $self:ident . $field:ident) => {
    // returns String
    $self
      .$field
      .as_ref()
      .map(|v| v.to_string())
      .unwrap_or(Err(Error::MissingField(stringify!($field))))?
  };
  (optional; $self:ident . $field:ident) => {
    // returns Option<String>
    $self.$field.as_ref().and_then(|v| v.to_string().ok())
  };
  (multi required; $self:ident . $field:ident) => {
    // returns Vec<String>
    $self
      .$field
      .as_ref()
      .map(|v| v.to_strings())
      .unwrap_or(Err(Error::MissingField(stringify!($field))))?
  };
  (multi optional; $self:ident . $field:ident) => {
    // returns Option<Vec<String>>
    $self.$field.as_ref().and_then(|v| v.to_strings().ok())
  };
}

impl FieldValues<'_> {
  fn build_rss_post(&self) -> Result<Post> {
    let mut item = rss::Item::default();

    let title = get_field!(required; self.title);
    item.set_title(Some(title));
    let link = get_field!(required; self.link);
    item.set_link(Some(link.clone()));
    let author = get_field!(optional; self.author);
    item.set_author(author);
    let description = get_field!(optional; self.description);
    item.set_description(description);
    let content = get_field!(optional; self.content_html);
    item.set_content(content);

    if let Some(pub_date) =
      get_field!(optional; self.pub_date).and_then(util::parse_date)
    {
      item.set_pub_date(pub_date.to_rfc2822());
    };

    let guid = if let Some(guid) = get_field!(optional; self.guid) {
      rss::Guid {
        permalink: guid == link,
        value: guid,
      }
    } else {
      rss::Guid {
        value: link,
        permalink: true,
      }
    };
    item.set_guid(Some(guid));

    if let Some(categories) = get_field!(multi optional; self.categories) {
      item.categories = categories
        .into_iter()
        .map(|name| rss::Category { name, domain: None })
        .collect();
    };

    if let Some(url) = get_field!(optional; self.enclosure_url) {
      let mime_type = get_field!(optional; self.enclosure_type)
        .unwrap_or_else(|| "application/octet-stream".to_owned());
      let length = get_field!(optional; self.enclosure_length)
        .unwrap_or_else(|| "0".into());
      item.enclosure = Some(rss::Enclosure {
        url,
        mime_type,
        length,
      });
    }

    Ok(Post::Rss(item))
  }

  fn build_atom_post(&self) -> Result<Post> {
    self
      .build_rss_post()
      .map(|post| post.into_format(FeedFormat::Atom))
  }
}

impl ConfigFeedMetaMap {
  fn parse(self) -> Result<ParsedFeedMetaMap, ConfigError> {
    let feed_map = ParsedFeedMetaMap {
      title: self.title.map(parse_field).transpose()?,
      link: self.link.map(parse_field).transpose()?,
      description: self.description.map(parse_field).transpose()?,
    };

    Ok(feed_map)
  }
}

impl ParsedFeedMetaMap {
  fn select<'a>(&self, root: &'a Value) -> Result<FeedMetaValues<'a>> {
    let meta = FeedMetaMap {
      title: select_field(root, &self.title, false)?,
      link: select_field(root, &self.link, false)?,
      description: select_field(root, &self.description, true)?,
    };

    Ok(meta)
  }
}

impl FeedMetaValues<'_> {
  fn apply(&self, feed: &mut Feed) -> Result<(), Error> {
    let title = get_field!(required; self.title);
    match feed {
      Feed::Rss(channel) => channel.title = title,
      Feed::Atom(atom) => atom.title = atom_syndication::Text::plain(title),
    }

    let link = get_field!(required; self.link);
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

    if let Some(description) = get_field!(optional; self.description) {
      match feed {
        Feed::Rss(channel) => {
          channel.description = description;
        }
        Feed::Atom(atom) => {
          atom.subtitle = Some(atom_syndication::Text::plain(description));
        }
      }
    }

    Ok(())
  }
}

fn parse_field(mut str: String) -> Result<ParsedField, ConfigError> {
  if str.starts_with("\\$") {
    str.remove(0);
    Ok(ParsedField::Const(str))
  } else if str.starts_with('$') {
    CompiledJsonPath::compile(&str)
      .map(ParsedField::JsonPath)
      .map_err(ConfigError::Message)
  } else {
    Ok(ParsedField::Const(str))
  }
}

fn select_field<'a>(
  root: &'a Value,
  field: &Option<ParsedField>,
  optional: bool,
) -> Result<Option<FieldValue<'a>>> {
  let Some(field) = field else {
    return Ok(None);
  };

  match (field, optional) {
    (ParsedField::Const(s), _) => Ok(Some(FieldValue::Const(s.clone()))),
    (ParsedField::JsonPath(compiled), true) => match compiled.select(root) {
      Ok(vals) if vals.len() == 1 => Ok(Some(FieldValue::Single(vals[0]))),
      Ok(vals) if vals.len() > 1 => Ok(Some(FieldValue::Multi(vals))),
      Ok(_vals) => Ok(None), // empty vals
      Err(JsonPathError::EmptyPath | JsonPathError::EmptyValue) => Ok(None),
      Err(err) => Err(err)?,
    },
    (ParsedField::JsonPath(compiled), false) => match compiled.select(root) {
      Ok(vals) if vals.len() == 1 => Ok(Some(FieldValue::Single(vals[0]))),
      Ok(vals) if vals.len() > 1 => Ok(Some(FieldValue::Multi(vals))),
      Ok(_vals) => Err(JsonPathError::EmptyValue)?, // empty vals
      Err(err) => Err(err)?,
    },
  }
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
  pub_date: "$.published_at"
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
