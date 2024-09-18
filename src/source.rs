use std::collections::{BTreeMap, HashMap};

use either::Either;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
  client::Client,
  feed::{Feed, FeedFormat},
  filter::FilterContext,
  server::EndpointParam,
  util::{ConfigError, Error, Result},
};

lazy_static::lazy_static! {
  static ref VAR_RE: Regex = Regex::new(r"\$\{(?<name>\w+)\}").unwrap();
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default,
)]
#[serde(untagged)]
/// # Feed source
pub enum SourceConfig {
  #[default]
  Dynamic,
  /// # Simple source
  ///
  /// A source that is a simple URL. A relative path (e.g. "/feed.xml")
  /// points to the current instance.
  Simple(String),
  /// # From scratch
  ///
  /// A source that is created from scratch
  FromScratch(FromScratch),
  /// # Templated source
  ///
  /// A source url that has placeholders that need to be filled in
  /// with values from the request.
  Templated(Templated),
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct SimpleSourceConfig(pub String);

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct Templated {
  /// The url of the source
  pub template: String,
  /// The placeholders. The key is the placeholder name and the value
  /// defines the value of the placeholder.
  // using BTreeMap instead of HashMap only because it implements Hash
  placeholders: BTreeMap<String, Placeholder>,
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct Placeholder {
  /// The default value of the placeholder. If not set, the placeholder
  /// is required.
  pub default: Option<String>,

  /// The regular expression that the placeholder must match. If not
  /// set, the placeholder can be any value. The validation is checked
  /// against the url-decoded value.
  pub validation: Option<String>,
}

impl Templated {
  fn to_regular_source(
    &self,
    params: &HashMap<String, String>,
  ) -> Result<Source> {
    let mut url = self.template.clone();

    for (name, placeholder) in &self.placeholders {
      let value = params
        .get(name)
        .or(placeholder.default.as_ref())
        .ok_or(Error::MissingSourceTemplatePlaceholder(name.clone()))?
        .clone();

      if let Some(validation) = &placeholder.validation {
        // already validated, so unwrap is safe
        let re = Regex::new(validation).unwrap();
        if !re.is_match(&value) {
          return Err(Error::SourceTemplateValidation {
            placeholder: name.clone(),
            validation: validation.clone(),
            input: value,
          });
        }
      }

      let encoded_value = urlencoding::encode(&value);
      url = url.replace(&format!("${{{name}}}"), &encoded_value);
    }

    SourceConfig::Simple(url)
      .try_into()
      .map_err(|e: ConfigError| e.into())
  }

  // https://foo.bar/${name}/baz -> https://foo.bar
  #[expect(unused)]
  pub fn base(&self) -> Option<String> {
    let mut url = Url::parse(&self.template).ok()?;
    let host = url.host_str()?;
    if VAR_RE.is_match(host) {
      return None;
    }
    url.set_fragment(None);
    url.set_query(None);
    url.set_path("");
    Some(url.to_string())
  }

  // used for rendering control
  pub fn fragments(
    &self,
  ) -> impl Iterator<Item = Either<&str, (&str, Option<&Placeholder>)>> + '_ {
    split_with_delimiter(&self.template, &VAR_RE).map(|e| match e {
      Either::Left(s) => Either::Left(s),
      Either::Right(cap) => {
        // SAFETY: name is guaranteed to be Some because the regex is
        // static.
        let name = &cap.name("name").unwrap();
        let placeholder = self.placeholders.get(name.as_str());
        Either::Right((&self.template[name.range()], placeholder))
      }
    })
  }
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub enum Source {
  Dynamic,
  AbsoluteUrl(Url),
  RelativeUrl(String),
  Templated(Templated),
  FromScratch(FromScratch),
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct FromScratch {
  /// The format of the feed
  pub format: FeedFormat,
  /// The title of the feed
  pub title: String,
  /// The url to the website
  pub link: Option<String>,
  /// A description of the feed
  pub description: Option<String>,
}

impl From<Url> for Source {
  fn from(url: Url) -> Self {
    Source::AbsoluteUrl(url)
  }
}

impl TryFrom<SimpleSourceConfig> for Source {
  type Error = ConfigError;

  fn try_from(config: SimpleSourceConfig) -> Result<Self, Self::Error> {
    SourceConfig::Simple(config.0).try_into()
  }
}

impl TryFrom<SourceConfig> for Source {
  type Error = ConfigError;

  fn try_from(config: SourceConfig) -> Result<Self, Self::Error> {
    match config {
      SourceConfig::Simple(url) if url.starts_with('/') => {
        Ok(Source::RelativeUrl(url))
      }
      SourceConfig::Simple(url) => {
        let url = Url::parse(&url)?;
        Ok(Source::AbsoluteUrl(url))
      }
      SourceConfig::FromScratch(config) => Ok(Source::FromScratch(config)),
      SourceConfig::Templated(config) => {
        validate_placeholders(&config)?;
        Ok(Source::Templated(config))
      }
      SourceConfig::Dynamic => Ok(Source::Dynamic),
    }
  }
}

fn validate_placeholders(config: &Templated) -> Result<(), ConfigError> {
  // Validation: placeholders must not be empty
  if config.placeholders.is_empty() {
    return Err(ConfigError::BadSourceTemplate(
      "placeholders must not be empty for templated source".into(),
    ));
  }

  // Validation: all placeholders must present in template
  for name in config.placeholders.keys() {
    if !config.template.contains(&format!("${{{name}}}")) {
      return Err(ConfigError::BadSourceTemplate(format!(
        "placeholder ${{{name}}} is not present in template",
      )));
    }
  }

  // Validation: all placeholder patterns in template must be
  // defined in placeholders
  for cap in VAR_RE.captures_iter(&config.template) {
    let name = &cap["name"];
    if !config.placeholders.contains_key(name) {
      return Err(ConfigError::BadSourceTemplate(format!(
        "placeholder ${{{name}}} is not defined",
      )));
    }
  }

  // Validation: all placeholder names must not be reserved words.
  const RESERVED_PARAMS: &[&str] = EndpointParam::all_fields();
  for name in config.placeholders.keys() {
    if RESERVED_PARAMS.contains(&name.as_str()) {
      return Err(ConfigError::BadSourceTemplate(format!(
        "placeholder `{name}` is a reserved word",
      )));
    }
  }

  // Validation: all parameter's validation regex must be valid regex
  for (name, placeholder) in &config.placeholders {
    if let Some(validation) = &placeholder.validation {
      Regex::new(validation).map_err(|e| {
        ConfigError::BadSourceTemplate(format!(
          "invalid regex for placeholder ${{{name}}}: {e}",
        ))
      })?;
    }
  }

  Ok(())
}

impl Source {
  pub async fn fetch_feed(
    &self,
    context: &FilterContext,
    client: Option<&Client>,
  ) -> Result<Feed> {
    let client = client.ok_or_else(|| Error::Message("client not set".into()));

    match self {
      Source::Dynamic => {
        let url = context.source().ok_or(Error::DynamicSourceUnspecified)?;
        client?.fetch_feed(url).await
      }
      Source::AbsoluteUrl(url) => client?.fetch_feed(url).await,
      Source::RelativeUrl(path) => {
        let url = context.base_expected()?.join(path)?;
        client?.fetch_feed(&url).await
      }
      Source::FromScratch(config) => Ok(Feed::from(config)),
      Source::Templated(template) => {
        let source = template.to_regular_source(context.extra_queries())?;
        Box::pin(source.fetch_feed(context, client.ok())).await
      }
    }
  }
}

fn split_with_delimiter<'a>(
  s: &'a str,
  re: &Regex,
) -> impl Iterator<Item = Either<&'a str, regex::Captures<'a>>> {
  let mut list = Vec::new();
  let mut last = 0;

  for cap in re.captures_iter(s) {
    // SAFETY: get(0) is guaranteed to be Some
    let full = cap.get(0).unwrap();
    let seg = &s[last..full.start()];
    if !seg.is_empty() {
      list.push(Either::Left(seg));
    }
    list.push(Either::Right(cap));
    last = full.end();
  }

  let tail = &s[last..];
  if !tail.is_empty() {
    list.push(Either::Left(tail));
  }
  list.into_iter()
}

#[cfg(test)]
mod test {
  use super::*;

  #[tokio::test]
  async fn test_fetch_feed_from_scratch_rss() {
    const YAML_CONFIG: &str = r#"
format: rss
title: "Test Feed"
link: "https://example.com"
description: "A test feed"
"#;

    let config: SourceConfig = serde_yaml::from_str(YAML_CONFIG).unwrap();
    let source = Source::try_from(config).unwrap();
    let ctx = FilterContext::new();
    let feed: Feed = source.fetch_feed(&ctx, None).await.unwrap();
    assert_eq!(feed.title(), "Test Feed");
    assert_eq!(feed.format(), FeedFormat::Rss);
  }

  #[tokio::test]
  async fn test_fetch_feed_from_scratch_atom() {
    const YAML_CONFIG: &str = r#"
format: atom
title: "Test Feed"
link: "https://example.com"
description: "A test feed"
"#;

    let config: SourceConfig = serde_yaml::from_str(YAML_CONFIG).unwrap();
    let source = Source::try_from(config).unwrap();
    let ctx = FilterContext::new();
    let feed: Feed = source.fetch_feed(&ctx, None).await.unwrap();
    assert_eq!(feed.title(), "Test Feed");
    assert_eq!(feed.format(), FeedFormat::Atom);
  }

  #[test]
  fn test_template_source_segmentation() {
    const YAML_CONFIG: &str = r#"
template: "https://example.com/${name1}/${name2}/feed.xml"
placeholders:
  name1:
    default: "default1"
  name2:
    default: "default2"
"#;

    let config: Templated = serde_yaml::from_str(YAML_CONFIG).unwrap();
    let fragments: Vec<_> = config.fragments().collect();
    assert_eq!(fragments.len(), 5);
    assert_eq!(fragments[0], Either::Left("https://example.com/"));
    assert_eq!(
      fragments[1],
      Either::Right((
        "name1",
        Some(&Placeholder {
          default: Some("default1".into()),
          validation: None,
        })
      ))
    );
    assert_eq!(fragments[2], Either::Left("/"));
    assert_eq!(
      fragments[3],
      Either::Right((
        "name2",
        Some(&Placeholder {
          default: Some("default2".into()),
          validation: None,
        })
      ))
    );
    assert_eq!(fragments[4], Either::Left("/feed.xml"));
  }
}
