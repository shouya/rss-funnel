use std::{hash::Hash, sync::Arc};

use axum::{
  body::Body, extract::Query, response::IntoResponse, Extension, Router,
};
use http::HeaderValue;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};
use url::Url;

use crate::util;

lazy_static::lazy_static! {
  static ref SIGN_KEY: Box<[u8]> = init_sign_key();
}

pub fn router() -> Router {
  info!("loaded image proxy: /_image");

  use tower_http::cors::AllowOrigin;
  let cors = CorsLayer::new()
    .allow_origin(AllowOrigin::any())
    .allow_methods(vec![http::Method::GET]);

  Router::new()
    .route("/_image", axum::routing::get(handler))
    .layer(cors)
    .layer(Extension(CachedClient::default()))
}

#[derive(Default, Clone)]
struct CachedClient {
  inner: Arc<RwLock<ClientInner>>,
}

impl CachedClient {
  async fn get(&self, config: &Config) -> reqwest::Client {
    if let Some(client) = self.inner.read().await.try_get(config).cloned() {
      return client;
    }

    let mut inner = self.inner.write().await;
    inner.update_and_get(config).clone()
  }
}

struct ClientInner {
  proxy: Option<String>,
  client: reqwest::Client,
}

impl Default for ClientInner {
  fn default() -> Self {
    Self {
      proxy: None,
      client: reqwest::Client::new(),
    }
  }
}

impl ClientInner {
  fn from_config(config: &Config) -> Self {
    let mut client = reqwest::Client::builder();
    if let Some(proxy) = &config.proxy {
      client = client.proxy(reqwest::Proxy::all(proxy).unwrap());
    }
    Self {
      proxy: config.proxy.clone(),
      client: client.build().unwrap(),
    }
  }

  fn try_get(&self, config: &Config) -> Option<&reqwest::Client> {
    if config.proxy == self.proxy {
      return Some(&self.client);
    }

    None
  }

  fn update_and_get(&mut self, config: &Config) -> &reqwest::Client {
    if config.proxy == self.proxy {
      return &self.client;
    }

    *self = Self::from_config(config);
    &self.client
  }
}

#[derive(Default, Debug, Deserialize, PartialEq, Eq)]
struct ProxyQuery {
  #[serde(rename = "url")]
  image_url: String,
  #[serde(default, flatten)]
  config: Config,
}

#[derive(Deserialize, PartialEq, Eq)]
struct SignatureQuery {
  #[serde(default)]
  sig: Option<String>,
}

#[derive(Error, Debug)]
pub enum Error {
  #[error("Invalid referer domain: {0}")]
  InvalidRefererDomain(Url),
  #[error("HTTP error: {0}")]
  Reqwest(#[from] reqwest::Error),
  #[error("User-Agent header contains invalid bytes: {0:?}")]
  UserAgentContainsInvalidBytes(HeaderValue),
  #[error("URL parse error: {0}")]
  UrlParse(#[from] url::ParseError),
  #[error("Missing signature")]
  MissingSignature,
  #[error("Bad signature")]
  BadSignature,
}

impl IntoResponse for Error {
  fn into_response(self) -> http::Response<Body> {
    use Error::*;
    warn!("{:?}", &self);

    match &self {
      MissingSignature => {
        (http::StatusCode::UNAUTHORIZED, self.to_string()).into_response()
      }
      BadSignature => {
        (http::StatusCode::FORBIDDEN, self.to_string()).into_response()
      }
      _ => (http::StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
        .into_response(),
    }
  }
}

type Result<T> = std::result::Result<T, Error>;

async fn handler(
  Extension(client): Extension<CachedClient>,
  Query(ProxyQuery { image_url, config }): Query<ProxyQuery>,
  Query(SignatureQuery { sig }): Query<SignatureQuery>,
  client_req: http::Request<Body>,
) -> Result<impl IntoResponse> {
  let sig = sig.ok_or(Error::MissingSignature)?;
  let expected_sig = signature(&config, &image_url, &SIGN_KEY);
  if sig != expected_sig {
    return Err(Error::BadSignature);
  }

  let client = client.get(&config).await;
  let mut proxy_req = client.get(&image_url);

  let user_agent = config.user_agent.unwrap_or_default();
  let user_agent = user_agent.calc_value(&client_req)?;
  if let Some(user_agent) = user_agent {
    proxy_req = proxy_req.header("user-agent", user_agent);
  }
  let referer = config.referer.unwrap_or_default();
  let referer = referer.calc_value(&image_url)?;
  if let Some(referer) = referer {
    proxy_req = proxy_req.header("referer", referer);
  }

  let res = proxy_req.send().await?;
  let res: http::Response<_> = res.into();
  let (mut parts, mut body) = res.into_parts();

  if !parts.status.is_success() {
    // in case body is a http page that may attempt to load external resources.
    body = reqwest::Body::from("");
    parts.headers.insert("content-length", "0".parse().unwrap());
  }

  let res = http::Response::from_parts(parts, body);

  Ok(res)
}

#[derive(
  Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default,
)]
#[serde(rename_all = "snake_case")]
enum Referer {
  None,
  #[default]
  ImageUrl,
  ImageUrlDomain,
  #[serde(untagged)]
  Fixed(String),
}

// Variant level #[serde(untagged)] is not supported by schemars, see
// https://github.com/GREsau/schemars/issues/222.
impl JsonSchema for Referer {
  fn schema_name() -> String {
    "referer".to_string()
  }

  fn is_referenceable() -> bool {
    false
  }

  fn json_schema(
    _gen: &mut schemars::gen::SchemaGenerator,
  ) -> schemars::schema::Schema {
    use schemars::schema::{
      InstanceType, Metadata, SchemaObject, SingleOrVec, SubschemaValidation,
    };

    let metadata = Metadata {
      title: Some("referer".to_string()),
      description: Some("Indicate what goes in the referer header when requesting the image url".to_string()),
      default: Some("image_url".into()),
      ..Default::default()
    };

    let subschemas = SubschemaValidation {
      any_of: Some(vec![
        SchemaObject {
          instance_type: Some(SingleOrVec::Single(Box::new(
            InstanceType::String,
          ))),
          enum_values: Some(vec![
            "none".into(),
            "image_url".into(),
            "image_url_domain".into(),
            "post_url".into(),
            "post_url_domain".into(),
          ]),
          ..Default::default()
        }
        .into(),
        SchemaObject {
          instance_type: Some(SingleOrVec::Single(Box::new(
            InstanceType::String,
          ))),
          ..Default::default()
        }
        .into(),
      ]),
      ..Default::default()
    };

    SchemaObject {
      metadata: Some(Box::new(metadata)),
      subschemas: Some(Box::new(subschemas)),
      ..Default::default()
    }
    .into()
  }
}

impl Referer {
  fn calc_value(&self, url: &str) -> Result<Option<String>> {
    match self {
      Self::None => Ok(None),
      Self::ImageUrl => Ok(Some(url.to_string())),
      Self::ImageUrlDomain => {
        let url = url::Url::parse(url)?;
        let domain = url
          .domain()
          .ok_or_else(|| Error::InvalidRefererDomain(url.clone()))?;
        Ok(Some(format!("{}://{}", url.scheme(), domain)))
      }
      Self::Fixed(s) => Ok(Some(s.clone())),
    }
  }
}

#[derive(
  Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default,
)]
#[serde(rename_all = "snake_case")]
enum UserAgent {
  None,
  #[default]
  Transparent,
  RssFunnel,
  #[serde(untagged)]
  Fixed(String),
}

impl UserAgent {
  fn calc_value<B>(&self, req: &http::Request<B>) -> Result<Option<String>> {
    match self {
      Self::None => Ok(None),
      Self::Transparent => {
        let Some(user_agent) = req.headers().get("user-agent") else {
          return Ok(None);
        };
        let user_agent = user_agent.to_str().map_err(|_| {
          Error::UserAgentContainsInvalidBytes(user_agent.clone())
        })?;
        Ok(Some(user_agent.to_string()))
      }
      Self::RssFunnel => Ok(Some(util::USER_AGENT.to_string())),
      Self::Fixed(s) => Ok(Some(s.clone())),
    }
  }
}

impl JsonSchema for UserAgent {
  fn schema_name() -> String {
    "user_agent".to_string()
  }

  fn is_referenceable() -> bool {
    false
  }

  fn json_schema(
    _gen: &mut schemars::gen::SchemaGenerator,
  ) -> schemars::schema::Schema {
    use schemars::schema::{
      InstanceType, Metadata, SchemaObject, SingleOrVec, SubschemaValidation,
    };

    let metadata = Metadata {
      title: Some("user_agent".to_string()),
      description: Some("Indicate what goes in the user-agent header when requesting the image url".to_string()),
      default: Some("transparent".into()),
      ..Default::default()
    };

    let subschemas = SubschemaValidation {
      any_of: Some(vec![
        SchemaObject {
          instance_type: Some(SingleOrVec::Single(Box::new(
            InstanceType::String,
          ))),
          enum_values: Some(vec![
            "none".into(),
            "transparent".into(),
            "rss_funnel".into(),
          ]),
          ..Default::default()
        }
        .into(),
        SchemaObject {
          instance_type: Some(SingleOrVec::Single(Box::new(
            InstanceType::String,
          ))),
          ..Default::default()
        }
        .into(),
      ]),
      ..Default::default()
    };

    SchemaObject {
      metadata: Some(Box::new(metadata)),
      subschemas: Some(Box::new(subschemas)),
      ..Default::default()
    }
    .into()
  }
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default,
)]
pub struct Config {
  referer: Option<Referer>,
  user_agent: Option<UserAgent>,
  proxy: Option<String>,
}

impl Config {
  pub fn to_query(&self, image_url: &str) -> String {
    let sig = signature(self, image_url, &SIGN_KEY);

    let mut params = vec![];
    if let Some(referer) = &self.referer {
      let referer = match referer {
        Referer::None => "none",
        Referer::ImageUrl => "image_url",
        Referer::ImageUrlDomain => "image_url_domain",
        Referer::Fixed(s) => &urlencoding::encode(s),
      };
      params.push(format!("referer={referer}"));
    }

    if let Some(user_agent) = &self.user_agent {
      let user_agent = match user_agent {
        UserAgent::None => "none",
        UserAgent::Transparent => "transparent",
        UserAgent::RssFunnel => "rss_funnel",
        UserAgent::Fixed(s) => &urlencoding::encode(s),
      };
      params.push(format!("user_agent={user_agent}"));
    }

    if let Some(proxy) = &self.proxy {
      let proxy = &urlencoding::encode(proxy);
      params.push(format!("proxy={proxy}"));
    }

    let image_url = &urlencoding::encode(image_url);
    params.push(format!("url={image_url}"));
    params.push(format!("sig={sig}"));

    params.join("&")
  }
}

fn init_sign_key() -> Box<[u8]> {
  if let Ok(key) = std::env::var("RSS_FUNNEL_IMAGE_PROXY_SIGN_KEY") {
    return key.into_bytes().into_boxed_slice();
  }

  Box::new(rand::random::<[u8; 32]>())
}

fn signature(config: &Config, url: &str, key: &[u8]) -> String {
  let mut hasher = blake3::Hasher::new();
  hasher.update(b"=key=");
  hasher.update(key);
  hasher.update(b"=config=");
  let config_bytes =
    serde_json::to_vec(config).expect("failed to serialize config");
  hasher.update(&config_bytes);
  hasher.update(b"=url=");
  hasher.update(url.as_bytes());
  let hash = hasher.finalize();
  hash.to_hex().as_str()[..16].to_string()
}

#[cfg(test)]
mod test {
  use super::*;
  use serde_json::json;

  #[test]
  fn test_parse_config() {
    let parsed: Config = serde_json::from_value(json!({})).unwrap();
    let expected = Config::default();
    assert_eq!(parsed, expected);

    let parsed: Config = serde_json::from_value(json!({
      "referer": "none"
    }))
    .unwrap();
    let expected = Config {
      referer: Some(Referer::None),
      ..Default::default()
    };
    assert_eq!(parsed, expected);

    let parsed: Config = serde_json::from_value(json!({
      "referer": "image_url_domain"
    }))
    .unwrap();
    let expected = Config {
      referer: Some(Referer::ImageUrlDomain),
      ..Default::default()
    };
    assert_eq!(parsed, expected);

    let parsed: Config = serde_json::from_value(json!({
      "referer": "http://example.com"
    }))
    .unwrap();
    let expected = Config {
      referer: Some(Referer::Fixed("http://example.com".to_string())),
      ..Default::default()
    };
    assert_eq!(parsed, expected);
  }
}
