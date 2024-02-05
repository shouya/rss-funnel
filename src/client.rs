mod cache;

use std::time::Duration;

use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{feed::Feed, util::Result};

use self::cache::{Response, ResponseCache};

#[cfg(test)]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct HttpFixture {
  url: String,
  content_type: String,
  content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientConfig {
  user_agent: Option<String>,
  accept: Option<String>,
  set_cookie: Option<String>,
  referer: Option<String>,
  cache_size: Option<usize>,
  #[serde(deserialize_with = "duration_str::deserialize_option_duration")]
  cache_ttl: Option<Duration>,
  #[serde(default = "default_timeout")]
  #[serde(deserialize_with = "duration_str::deserialize_duration")]
  timeout: Duration,
  /// Sometimes the feed doesn't specify a
  #[serde(default)]
  assume_content_type: Option<String>,
}

impl Default for ClientConfig {
  fn default() -> Self {
    Self {
      user_agent: None,
      accept: None,
      set_cookie: None,
      referer: None,
      timeout: default_timeout(),
      cache_size: None,
      cache_ttl: None,
      assume_content_type: None,
    }
  }
}

impl ClientConfig {
  fn to_builder(&self) -> reqwest::ClientBuilder {
    let mut builder = reqwest::Client::builder();

    if let Some(user_agent) = &self.user_agent {
      builder = builder.user_agent(user_agent);
    } else {
      builder = builder.user_agent(crate::util::USER_AGENT);
    }

    let mut header_map = HeaderMap::new();
    if let Some(accept) = &self.accept {
      header_map
        .append("Accept", accept.try_into().expect("invalid Accept value"));
    }

    if let Some(set_cookie) = &self.set_cookie {
      header_map.append(
        "Set-Cookie",
        set_cookie.try_into().expect("invalid Set-Cookie value"),
      );
    }

    if let Some(referer) = &self.referer {
      header_map.append(
        "Referer",
        referer.try_into().expect("invalid Referer value"),
      );
    }

    if !header_map.is_empty() {
      builder = builder.default_headers(header_map);
    }

    builder = builder.timeout(self.timeout);

    builder
  }

  pub fn build(&self, default_cache_ttl: Duration) -> Result<Client> {
    let reqwest_client = self.to_builder().build()?;
    let client = Client::new(
      self.cache_size.unwrap_or(0),
      self.cache_ttl.unwrap_or(default_cache_ttl),
      reqwest_client,
      self.assume_content_type.clone(),
    );
    Ok(client)
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
    let content = resp.text()?;

    let feed = match content_type.as_deref() {
      Some("text/html") => Feed::from_html_content(&content, source)?,
      Some("application/rss+xml") => Feed::from_rss_content(&content)?,
      Some("application/atom+xml") => Feed::from_atom_content(&content)?,
      Some("application/xml") | Some("text/xml") => {
        Feed::from_xml_content(&content)?
      }
      x => todo!("{:?}", x),
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

    let resp = f(self.client.get(url.clone())).send().await?;
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

fn default_timeout() -> Duration {
  Duration::from_secs(10)
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
