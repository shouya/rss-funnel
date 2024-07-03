use axum::{body::Body, extract::Query, response::IntoResponse};
use http::HeaderValue;
use serde::Deserialize;
use thiserror::Error;
use url::Url;

use crate::util;

#[derive(Default, Debug, Deserialize, PartialEq, Eq)]
struct ProxyQuery {
  #[serde(rename = "url")]
  image_url: String,
  #[serde(default, flatten)]
  config: Config,
}

#[derive(Error, Debug)]
enum Error {
  #[error("Invalid referer domain: {0}")]
  InvalidRefererDomain(Url),
  #[error("HTTP error: {0}")]
  Reqwest(#[from] reqwest::Error),
  #[error("Header contains invalid bytes: {header}: {value:?}")]
  HeaderContainsInvalidBytes { header: String, value: HeaderValue },
  #[error("URL parse error: {0}")]
  UrlParse(#[from] url::ParseError),
}

impl IntoResponse for Error {
  fn into_response(self) -> http::Response<Body> {
    http::Response::builder()
      .status(http::StatusCode::INTERNAL_SERVER_ERROR)
      .body(Body::from(self.to_string()))
      .unwrap()
  }
}

type Result<T> = std::result::Result<T, Error>;

pub async fn handler(
  Query(ProxyQuery { image_url, config }): Query<ProxyQuery>,
  client_req: http::Request<Body>,
) -> Result<impl IntoResponse> {
  let mut client = reqwest::Client::builder();
  if let Some(proxy) = q.config.proxy {
    client = client.proxy(reqwest::Proxy::all(proxy).unwrap());
  }
  let client = client.build()?;

  let mut proxy_req = client.get(&image_url);

  let user_agent = config.user_agent.unwrap_or_default();
  let user_agent = user_agent.calc_value(&client)?;
  if let Some(user_agent) = user_agent {
    proxy_req.header("user-agent", user_agent);
  }
  let referer = config.referer.unwrap_or_default();
  let referer = referer.calc_value(&image_url, &client_req)?;
  if let Some(referer) = referer {
    proxy_req.header("referer", referer);
  }

  let res = proxy_req.send().await?;
  let mut res = http::Response::builder()
    .status(res.status())
    .header("content-type", res.headers().get("content-type").unwrap())
    .body(res.bytes().await?)
    .unwrap();
}

#[derive(Default, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Referer {
  #[default]
  None,
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
          .ok_or_else(|| Error::InvalidRefererDomain(url))?;
        Ok(Some(format!("{}://{}", url.scheme(), domain)))
      }
      Self::Transparent => {
        let Some(referer) = req.headers().get("referer") else {
          return Ok(None);
        };
        Ok(Some(referer.to_str()?.to_string()))
      }
      Self::TransparentDomain => {
        let Some(referer) = req.headers().get("referer") else {
          return Ok(None);
        };
        let referer = referer.to_str()?;
        let url = url::Url::parse(referer)?;
        let domain = url.domain()?;
        Ok(Some(format!("{}://{}", url.scheme(), domain)))
      }
      Self::Fixed(s) => Ok(Some(s.clone())),
    }
  }
}

#[derive(Default, Debug, Deserialize, PartialEq, Eq)]
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
        Ok(Some(user_agent.to_str()?.to_string()))
      }
      Self::RssFunnel => Ok(Some(util::USER_AGENT.to_string())),
      Self::Fixed(s) => Ok(Some(s.clone())),
    }
  }
}

#[derive(Default, Debug, Deserialize, PartialEq, Eq)]
struct Config {
  referer: Option<Referer>,
  user_agent: Option<String>,
  proxy: Option<String>,
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
