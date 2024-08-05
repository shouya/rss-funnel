use std::collections::{BTreeMap, HashMap};

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

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(untagged)]
/// # Feed source
pub enum SourceConfig {
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
pub struct Templated {
  /// The url of the source
  template: String,
  /// The placeholders. The key is the placeholder name and the value
  /// defines the value of the placeholder.
  // using BTreeMap instead of HashMap only because it implements Hash
  placeholders: BTreeMap<String, Placeholder>,
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
struct Placeholder {
  /// The default value of the placeholder. If not set, the placeholder
  /// is required.
  default: Option<String>,

  /// The regular expression that the placeholder must match. If not
  /// set, the placeholder can be any value. The validation is checked
  /// against the url-decoded value.
  validation: Option<String>,
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
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub enum Source {
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
        "placeholder %{{{name}}}% is not present in template",
      )));
    }
  }

  // Validation: all placeholder patterns in template must be
  // defined in placeholders
  lazy_static::lazy_static! {
    static ref RE: Regex = Regex::new(r"$\{(?<name>\w+)\}").unwrap();
  }
  for cap in RE.captures_iter(&config.template) {
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
    if let Source::FromScratch(config) = self {
      let feed = Feed::from(config);
      return Ok(feed);
    }

    if let Source::Templated(config) = self {
      let source = config.to_regular_source(context.extra_queries())?;
      return Box::pin(source.fetch_feed(context, client)).await;
    }

    let source_url = match self {
      Source::AbsoluteUrl(url) => url.clone(),
      Source::RelativeUrl(path) => {
        let base = context.base_expected()?;
        base.join(path)?
      }
      Source::Templated(_) | Source::FromScratch(_) => unreachable!(),
    };

    let client =
      client.ok_or_else(|| Error::Message("client not set".into()))?;
    client.fetch_feed(&source_url).await
  }
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
}
