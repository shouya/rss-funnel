use std::borrow::Cow;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::warn;
use url::Url;

use super::{FeedFilter, FeedFilterConfig, FilterContext};
use crate::{feed::Feed, ConfigError, Error, Result};

const IMAGE_PROXY_ROUTE: &str = "_image";

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct Config {
  /// Only rewrite images whose url matches one of the given
  /// domains. Globbing is supported: "*.example.com" matches
  /// "foo.example.com" but not "example.com".
  domains: Option<Vec<String>>,
  /// Only rewrite urls of <img> tags matching the following CSS
  /// selector.
  selector: Option<String>,
  #[serde(flatten)]
  settings: ProxySettings,
}

impl Config {
  fn selector(&self) -> String {
    self.selector.clone().unwrap_or_else(|| "img".to_string())
  }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
enum ProxySettings {
  External(ExternalProxySettings),
  #[serde(untagged)]
  Internal(crate::server::image_proxy::Config),
}

impl ProxySettings {
  fn rewrite_image_url(
    &self,
    ctx: &FilterContext,
    image_url: Url,
  ) -> Result<String> {
    match self {
      ProxySettings::External(settings) => {
        let urlencode = settings
          .urlencode
          .unwrap_or_else(|| settings.base.as_str().ends_with('='));
        let image_url = if urlencode {
          urlencoding::encode(image_url.as_str())
        } else {
          Cow::Borrowed(image_url.as_str())
        };
        let base = settings.base.as_str();

        Ok(format!("{base}{image_url}"))
      }
      ProxySettings::Internal(proxy) => {
        let query = proxy.to_query(image_url.as_str());
        let app_base = ctx.base_expected()?;
        match app_base.join(&format!("{IMAGE_PROXY_ROUTE}?{query}")) {
          Ok(url) => Ok(url.to_string()),
          Err(e) => {
            warn!("Failed to rewrite html for image proxy: {e}");
            Ok(image_url.to_string())
          }
        }
      }
    }
  }
}

impl JsonSchema for ProxySettings {
  fn schema_name() -> String {
    "ImageProxySettings".to_owned()
  }

  fn json_schema(
    gen: &mut schemars::gen::SchemaGenerator,
  ) -> schemars::schema::Schema {
    use schemars::schema::{
      InstanceType, Metadata, Schema, SchemaObject, SingleOrVec,
      SubschemaValidation,
    };

    let variant1_metadata = Metadata {
      title: Some("ExternalProxySettings".to_owned()),
      description: Some("Settings for an external image proxy.".to_owned()),
      ..Metadata::default()
    };
    let variant1_inner = ExternalProxySettings::json_schema(gen);
    let mut variant1 = SchemaObject {
      instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Object))),
      metadata: Some(Box::new(variant1_metadata)),
      ..Default::default()
    };
    variant1
      .object()
      .properties
      .insert("external".to_string(), variant1_inner);
    variant1.object().required =
      vec!["external".to_string()].into_iter().collect();
    let variant1: Schema = variant1.into();

    let variant2 = crate::server::image_proxy::Config::json_schema(gen);
    let subschema = SubschemaValidation {
      any_of: Some(vec![variant1, variant2]),
      ..Default::default()
    };

    let metadata = Metadata {
      title: Some("ImageProxySettings".to_owned()),
      description: Some("Settings for the image proxy.".to_owned()),
      ..Metadata::default()
    };

    SchemaObject {
      metadata: Some(Box::new(metadata)),
      subschemas: Some(Box::new(subschema)),
      ..Default::default()
    }
    .into()
  }
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
struct ExternalProxySettings {
  /// The base URL to append the images to.
  base: Url,
  /// Whether to urlencode the images urls before appending them to
  /// the base. If base ends with a "=", this option defaults to true,
  /// otherwise it defaults to false.
  urlencode: Option<bool>,
}

#[async_trait::async_trait]
impl FeedFilterConfig for Config {
  type Filter = ImageProxy;

  async fn build(self) -> Result<Self::Filter, ConfigError> {
    Ok(ImageProxy { config: self })
  }
}

pub struct ImageProxy {
  config: Config,
}

#[async_trait::async_trait]
impl FeedFilter for ImageProxy {
  async fn run(&self, ctx: &mut FilterContext, mut feed: Feed) -> Result<Feed> {
    let mut posts = feed.take_posts();

    for post in &mut posts {
      for body in post.bodies_mut() {
        if let Some(new_body) = rewrite_html(ctx, &self.config, body) {
          *body = new_body;
        }
      }
    }

    feed.set_posts(posts);
    Ok(feed)
  }

  fn cache_granularity(&self) -> super::CacheGranularity {
    super::CacheGranularity::FeedAndPost
  }
}

fn rewrite_html(
  ctx: &FilterContext,
  config: &Config,
  html: &str,
) -> Option<String> {
  use glob_match::glob_match;
  use lol_html::{element, RewriteStrSettings};

  let selector = config.selector();
  let matches_domain = |url: &Url| match (&config.domains, url.domain()) {
    (None, _) => true,
    (_, None) => false,
    (Some(ref domains), Some(domain)) => {
      domains.iter().any(|pat| glob_match(pat, domain))
    }
  };

  let rewrite = element!(selector, |el| {
    rewrite_img_elem(ctx, config, &matches_domain, el)?;
    Ok(())
  });

  let res = lol_html::rewrite_str(
    html,
    RewriteStrSettings {
      element_content_handlers: vec![rewrite],
      ..RewriteStrSettings::default()
    },
  );

  match res {
    Ok(html) => Some(html),
    Err(e) => {
      warn!("Failed to rewrite html: {e}");
      None
    }
  }
}

fn rewrite_img_elem(
  ctx: &FilterContext,
  config: &Config,
  matches_domain: &impl Fn(&Url) -> bool,
  el: &mut lol_html::html_content::Element<'_, '_>,
) -> Result<()> {
  lazy_static::lazy_static! {
    static ref URL_REGEX: regex::Regex =
      regex::Regex::new(r"https?://[^\s]+\b").unwrap();
  }

  let new_src = el
    .get_attribute("src")
    .iter()
    .filter_map(|src| Url::parse(src).ok())
    .filter(matches_domain)
    .map(|url| config.settings.rewrite_image_url(ctx, url))
    .next()
    .transpose()?;

  if let Some(new_src) = new_src {
    // safe to unwrap because we know the attribute name is valid
    el.set_attribute("src", &new_src).unwrap();
  }

  let new_srcset = el
    .get_attribute("srcset")
    .map(|srcset| {
      let sources: Vec<&str> =
        srcset.trim().split(',').map(str::trim).collect();
      let mut new_sources: Vec<String> = Vec::new();
      for source in sources {
        let source = source.trim();
        let split = source.split_once(' ');
        let url = split.map_or(source, |(a, _b)| a);
        let url = Url::parse(url).ok().filter(matches_domain);
        let remaining = split.map(|(_a, b)| b);

        let new_url = url
          .map(|url| config.settings.rewrite_image_url(ctx, url))
          .transpose()?
          .map(|url| {
            if let Some(remaining) = remaining {
              format!("{url} {remaining}")
            } else {
              url
            }
          })
          .unwrap_or_else(|| source.to_string());

        new_sources.push(new_url);
      }

      Ok::<_, Error>(new_sources.join(", "))
    })
    .transpose()?;

  if let Some(new_srcset) = new_srcset {
    // safe to unwrap because we know the attribute name is valid
    el.set_attribute("srcset", &new_srcset).unwrap();
  }

  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_src_rewrite() {
    let ctx = filter_context_fixture();
    let config = config_fixture();

    let html = r#"
      <p><img src="http://a1.com/a.jpg"></p>
      <img class="proxy" src="http://a2.com/a.jpg">
      <img class="proxy" src="http://a3.com/a.jpg">
    "#;

    let expected = r#"
      <p><img src="http://a1.com/a.jpg"></p>
      <img class="proxy" src="http://app.com/_image?url=http%3A%2F%2Fa2.com%2Fa.jpg">
      <img class="proxy" src="http://a3.com/a.jpg">
    "#;

    let actual = rewrite_html(&ctx, &config, html).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn test_srcset_rewrite() {
    let ctx = filter_context_fixture();
    let config = config_fixture();

    let html = r#"
      <img class="proxy"
           src="http://a1.com/a.jpg"
           srcset="http://a1.com/a.jpg 1x,
                   http://a2.com/a.jpg 2x,
                   http://a3.com/a.jpg 3x">
      <img class="proxy"
           src="http://a1.com/a.jpg"
           srcset="http://a1.com/a.jpg,
                   http://a2.com/a.jpg,
                   http://a3.com/a.jpg">
    "#;
    let expected = r#"
      <img class="proxy"
           src="http://app.com/_image?url=http%3A%2F%2Fa1.com%2Fa.jpg"
           srcset="http://app.com/_image?url=http%3A%2F%2Fa1.com%2Fa.jpg 1x,
                   http://app.com/_image?url=http%3A%2F%2Fa2.com%2Fa.jpg 2x,
                   http://a3.com/a.jpg 3x">
      <img class="proxy"
           src="http://app.com/_image?url=http%3A%2F%2Fa1.com%2Fa.jpg"
           srcset="http://app.com/_image?url=http%3A%2F%2Fa1.com%2Fa.jpg,
                   http://app.com/_image?url=http%3A%2F%2Fa2.com%2Fa.jpg,
                   http://a3.com/a.jpg">
    "#;

    let actual = rewrite_html(&ctx, &config, html).unwrap();
    assert_eq!(squeeze_spaces(&actual), squeeze_spaces(expected));
  }

  fn filter_context_fixture() -> FilterContext {
    let mut ctx = FilterContext::new();
    ctx.set_base(Url::parse("http://app.com").unwrap());
    ctx
  }

  fn config_fixture() -> Config {
    let proxy_conf =
      crate::server::image_proxy::Config::default().without_signature();

    Config {
      domains: Some(vec!["a1.com".to_string(), "a2.com".to_string()]),
      selector: Some("img.proxy".to_string()),
      settings: ProxySettings::Internal(proxy_conf),
    }
  }

  fn squeeze_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<&str>>().join(" ")
  }
}
