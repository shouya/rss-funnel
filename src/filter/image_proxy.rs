use std::borrow::Cow;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::warn;
use url::Url;

use super::{FeedFilter, FeedFilterConfig, FilterContext};
use crate::{feed::Feed, util::ConfigError, Result};

const IMAGE_PROXY_ROUTE: &str = "/_image";

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
  fn is_internal(&self) -> bool {
    matches!(self, ProxySettings::Internal(_))
  }

  fn rewrite_image_url(&self, ctx: &FilterContext, image_url: Url) -> String {
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

        format!("{}{}", base, image_url)
      }
      ProxySettings::Internal(proxy) => {
        let query = proxy.to_query(image_url.as_str());
        let app_base = ctx.base();

        match app_base.join(&format!("{}?{}", IMAGE_PROXY_ROUTE, query)) {
          Ok(url) => url.to_string(),
          Err(e) => {
            warn!("Failed to rewrite image url: {}", e);
            image_url.to_string()
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

    for post in posts.iter_mut() {
      for body in post.bodies_mut() {
        if let Some(new_body) = rewrite_html(ctx, &self.config, body) {
          *body = new_body;
        }
      }
    }

    feed.set_posts(posts);
    Ok(feed)
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
    let new_src = el
      .get_attribute("src")
      .iter()
      .filter_map(|src| Url::parse(src).ok())
      .filter(matches_domain)
      .map(|url| config.settings.rewrite_image_url(ctx, url))
      .next();

    if let Some(new_src) = new_src {
      el.set_attribute("src", &new_src)?;
    }

    Ok(())
  });

  lol_html::rewrite_str(
    html,
    RewriteStrSettings {
      element_content_handlers: vec![rewrite],
      ..RewriteStrSettings::default()
    },
  )
  .ok()
}
