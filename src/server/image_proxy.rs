use std::sync::Arc;

use axum::{
  body::Body, extract::Query, response::IntoResponse, Extension, Router,
};
use http::HeaderValue;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::warn;
use url::Url;

use crate::util;

pub fn router() -> Router {
  if !util::is_env_set("RSS_FUNNEL_IMAGE_PROXY") {
    return Router::new();
  }

  use tower_http::cors::AllowOrigin;
  let cors = CorsLayer::new()
    .allow_origin(AllowOrigin::any())
    .allow_methods(vec![http::Method::GET]);

  Router::new()
    .route("/", axum::routing::get(handler))
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

#[derive(Error, Debug)]
pub enum Error {
  #[error("Invalid referer domain: {0}")]
  InvalidRefererDomain(Url),
  #[error("HTTP error: {0}")]
  Reqwest(#[from] reqwest::Error),
  #[error("Referer header contains invalid bytes: {0:?}")]
  RefererContainsInvalidBytes(HeaderValue),
  #[error("User-Agent header contains invalid bytes: {0:?}")]
  UserAgentContainsInvalidBytes(HeaderValue),
  #[error("URL parse error: {0}")]
  UrlParse(#[from] url::ParseError),
}

impl IntoResponse for Error {
  fn into_response(self) -> http::Response<Body> {
    warn!("{:?}", &self);
    http::Response::builder()
      .status(http::StatusCode::INTERNAL_SERVER_ERROR)
      .body(Body::from(self.to_string()))
      .unwrap()
  }
}

type Result<T> = std::result::Result<T, Error>;

async fn handler(
  Extension(client): Extension<CachedClient>,
  Query(ProxyQuery { image_url, config }): Query<ProxyQuery>,
  client_req: http::Request<Body>,
) -> Result<impl IntoResponse> {
  let client = client.get(&config).await;
  let mut proxy_req = client.get(&image_url);

  let user_agent = config.user_agent.unwrap_or_default();
  let user_agent = user_agent.calc_value(&client_req)?;
  if let Some(user_agent) = user_agent {
    proxy_req = proxy_req.header("user-agent", user_agent);
  }
  let referer = config.referer.unwrap_or_default();
  let referer = referer.calc_value(&image_url, &client_req)?;
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

#[derive(Default, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Referer {
  None,
  #[default]
  ImageUrl,
  ImageUrlDomain,
  Transparent,
  TransparentDomain,
  #[serde(untagged)]
  Fixed(String),
}

impl Referer {
  fn calc_value<B>(
    &self,
    url: &str,
    req: &http::Request<B>,
  ) -> Result<Option<String>> {
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
      Self::Transparent => {
        let Some(referer) = req.headers().get("referer") else {
          return Ok(None);
        };
        let referer = referer
          .to_str()
          .map_err(|_| Error::RefererContainsInvalidBytes(referer.clone()))?;

        Ok(Some(referer.to_string()))
      }
      Self::TransparentDomain => {
        let Some(referer) = req.headers().get("referer") else {
          return Ok(None);
        };
        let referer = referer
          .to_str()
          .map_err(|_| Error::RefererContainsInvalidBytes(referer.clone()))?;
        let url = url::Url::parse(referer)?;
        let domain = url
          .domain()
          .ok_or_else(|| Error::InvalidRefererDomain(url.clone()))?;
        Ok(Some(format!("{}://{}", url.scheme(), domain)))
      }
      Self::Fixed(s) => Ok(Some(s.clone())),
    }
  }
}

#[derive(Default, Debug, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Default, Debug, Deserialize, PartialEq, Eq)]
pub struct Config {
  referer: Option<Referer>,
  user_agent: Option<UserAgent>,
  proxy: Option<String>,
}

impl Config {
  pub fn to_query(&self, image_url: &str) -> String {
    let mut params = vec![];
    if let Some(referer) = &self.referer {
      let referer = match referer {
        Referer::None => "none",
        Referer::ImageUrl => "image_url",
        Referer::ImageUrlDomain => "image_url_domain",
        Referer::Transparent => "transparent",
        Referer::TransparentDomain => "transparent_domain",
        Referer::Fixed(s) => s,
      };
      params.push(format!("referer={referer}"));
    }

    if let Some(user_agent) = &self.user_agent {
      let user_agent = match user_agent {
        UserAgent::None => "none",
        UserAgent::Transparent => "transparent",
        UserAgent::RssFunnel => "rss_funnel",
        UserAgent::Fixed(s) => s,
      };
      params.push(format!("user_agent={user_agent}"));
    }

    if let Some(proxy) = &self.proxy {
      params.push(format!("proxy={proxy}"));
    }

    params.push(format!("url={image_url}"));
    params.join("&")
  }
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
      "referer": "transparent"
    }))
    .unwrap();
    let expected = Config {
      referer: Some(Referer::Transparent),
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
