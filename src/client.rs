mod cache;

use std::time::Duration;

use reqwest::header::HeaderMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
  feed::Feed,
  ConfigError, Error, Result,
};

use self::cache::{Response, ResponseCache};

#[cfg(test)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct HttpFixture {
  url: String,
  content_type: String,
  content: String,
}

#[serde_with::skip_serializing_none]
#[derive(
  JsonSchema, Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, Hash,
)]
pub struct ClientConfig {
  /// The "user-agent" header to send with requests
  #[serde(default)]
  pub user_agent: Option<String>,
  /// The "accept" header to send with requests
  #[serde(default)]
  pub accept: Option<String>,
  /// The "cookie" header to send with requests (Deprecated, specify "cookie" field instead)
  #[serde(default)]
  pub set_cookie: Option<String>,
  /// The "cookie" header to send with requests
  #[serde(default)]
  pub cookie: Option<String>,
  /// The "referer" header to send with requests
  #[serde(default)]
  pub referer: Option<String>,
  /// Ignore tls error
  #[serde(default)]
  pub accept_invalid_certs: bool,
  /// The maximum number of cached responses
  #[serde(default)]
  pub cache_size: Option<usize>,
  /// The maximum time a response is kept in the cache (Format: "4s",
  /// 10m", "1h", "1d")
  #[serde(default)]
  #[serde(deserialize_with = "duration_str::deserialize_option_duration")]
  #[schemars(with = "String")]
  pub cache_ttl: Option<Duration>,
  /// Request timeout (Format: "4s", "10m", "1h", "1d")
  #[serde(default)]
  #[serde(deserialize_with = "duration_str::deserialize_option_duration")]
  #[schemars(with = "String")]
  pub timeout: Option<Duration>,
  /// Sometimes the feed doesn't report a correct content type, so we
  /// need to override it.
  #[serde(default)]
  pub assume_content_type: Option<String>,
  /// The proxy to use for requests
  /// (Format: "http://user:pass@host:port", "socks5://user:pass@host:port")
  #[serde(default)]
  pub proxy: Option<String>,
}

impl ClientConfig {
  pub fn get_cache_size(&self) -> usize {
    self.cache_size.unwrap_or(64)
  }
  pub fn get_cache_ttl(&self, default_cache_ttl: Duration) -> Duration {
    self.cache_ttl.unwrap_or(default_cache_ttl)
  }

  fn to_builder(&self) -> Result<reqwest::ClientBuilder, ConfigError> {
    let mut builder = reqwest::Client::builder();

    if let Some(user_agent) = &self.user_agent {
      builder = builder.user_agent(user_agent);
    } else {
      builder = builder.user_agent(crate::util::USER_AGENT);
    }

    let mut header_map = HeaderMap::new();
    if let Some(accept) = &self.accept {
      header_map.append("Accept", accept.try_into()?);
    }

    if let Some(cookie) = &self.cookie {
      header_map.append("Cookie", cookie.try_into()?);
    } else if let Some(set_cookie) = &self.set_cookie {
      header_map.append("Cookie", set_cookie.try_into()?);
    }

    if let Some(referer) = &self.referer {
      header_map.append("Referer", referer.try_into()?);
    }

    if !header_map.is_empty() {
      builder = builder.default_headers(header_map);
    }

    if self.accept_invalid_certs {
      builder = builder.danger_accept_invalid_certs(true);
    }

    let default_timeout = Duration::from_secs(10);
    builder = builder.timeout(self.timeout.unwrap_or(default_timeout));

    if let Some(proxy) = &self.proxy {
      builder = builder.proxy(reqwest::Proxy::all(proxy)?);
    }

    Ok(builder)
  }

  pub fn build(
    &self,
    default_cache_ttl: Duration,
  ) -> Result<Client, ConfigError> {
    let reqwest_client = self.to_builder()?.build()?;
    let client = Client::new(
      self.get_cache_size(),
      self.get_cache_ttl(default_cache_ttl),
      reqwest_client,
      self.assume_content_type.clone(),
    );
    Ok(client)
  }

  pub fn to_yaml(&self) -> Result<String, ConfigError> {
    Ok(serde_yaml::to_string(self)?)
  }
}

pub struct Client {
  cache: ResponseCache,
  client: reqwest::Client,
  assume_content_type: Option<String>,
}

impl Client {
  fn new(
    cache_size: usize,
    cache_ttl: Duration,
    client: reqwest::Client,
    assume_content_type: Option<String>,
  ) -> Self {
    Self {
      cache: ResponseCache::new(cache_size, cache_ttl),
      client,
      assume_content_type,
    }
  }

  const ACCEPTED_CONTENT_TYPES: [&'static str; 6] = [
    "application/xml",
    "text/xml",
    "application/rss+xml",
    "application/atom+xml",
    "text/html",
    "*/*",
  ];

  pub async fn fetch_feed(&self, source: &Url) -> Result<Feed> {
    let resp = self
      .get_with(source, |builder| {
        builder.header("Accept", Self::ACCEPTED_CONTENT_TYPES.join(", "))
      })
      .await?
      .error_for_status()?;

    let content_type = resp.content_type().map(|x| x.essence_str().to_owned());

    let feed = match content_type.as_deref() {
      Some("text/html") => Feed::from_html_content(&resp.text()?, source)?,
      Some("application/rss+xml") => Feed::from_rss_content(resp.body())?,
      Some("application/atom+xml") => Feed::from_atom_content(resp.body())?,
      Some("application/xml") | Some("text/xml") => {
        Feed::from_xml_content(resp.body())?
      }
      Some(format) => Err(Error::UnsupportedFeedFormat(format.into()))?,
      None => Feed::from_xml_content(resp.body())?,
    };

    Ok(feed)
  }

  pub async fn get(&self, url: &Url) -> Result<Response> {
    self.get_with(url, |req| req).await
  }

  pub async fn get_with(
    &self,
    url: &Url,
    f: impl FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
  ) -> Result<Response> {
    #[cfg(test)]
    if url.scheme() == "fixture" {
      return Ok(Response::from_fixture(url));
    }

    if let Some(resp) = self.cache.get_cached(url) {
      return Ok(resp);
    }

    let req_builder = self.client.get(url.clone());
    let req_builder = f(req_builder);

    let resp = req_builder.send().await?;
    let resp = Response::from_reqwest_resp(resp).await?;
    let resp = self.modify_resp(resp);
    self.cache.insert(url.clone(), resp.clone());
    Ok(resp)
  }

  fn modify_resp(&self, mut resp: Response) -> Response {
    let Some(assume_content_type) = &self.assume_content_type else {
      return resp;
    };

    resp.set_content_type(assume_content_type);
    resp
  }

  #[cfg(test)]
  pub fn insert(&self, url: Url, resp: Response) {
    self.cache.insert(url, resp);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_client_cache() {
    let client =
      Client::new(1, Duration::from_secs(1), reqwest::Client::new(), None);
    let url = Url::parse("http://example.com").unwrap();
    let body: Box<str> = "foo".into();
    let response = Response::new(
      url.clone(),
      reqwest::StatusCode::OK,
      HeaderMap::new(),
      body.into(),
    );

    client.insert(url.clone(), response.clone());
    let actual = client.get(&url).await.unwrap();
    let expected = response;

    assert_eq!(actual.url(), expected.url());
    assert_eq!(actual.status(), expected.status());
    assert_eq!(actual.headers(), expected.headers());
    assert_eq!(actual.body(), expected.body());
  }

  const YT_SCISHOW_FEED_URL: &str = "https://www.youtube.com/feeds/videos.xml?channel_id=UCZYTClx2T1of7BRZ86-8fow";

  #[tokio::test]
  async fn test_client() {
    let client =
      Client::new(0, Duration::from_secs(1), reqwest::Client::new(), None);
    let url = Url::parse(YT_SCISHOW_FEED_URL).unwrap();
    let resp = client.get(&url).await.unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    assert_eq!(
      resp.content_type().unwrap().to_string(),
      "text/xml; charset=utf-8"
    );
    assert!(resp.text().unwrap().contains("<title>SciShow</title>"));
  }
}
