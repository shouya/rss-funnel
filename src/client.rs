mod cache;

use std::time::Duration;

use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::util::Result;

use self::cache::{Response, ResponseCache};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientConfig {
  user_agent: Option<String>,
  accept: Option<String>,
  set_cookie: Option<String>,
  referer: Option<String>,
  cache_size: Option<usize>,
  #[serde(deserialize_with = "duration_str::deserialize_duration")]
  #[serde(default = "default_cache_ttl")]
  cache_ttl: Duration,
  #[serde(default = "default_timeout")]
  #[serde(deserialize_with = "duration_str::deserialize_duration")]
  timeout: Duration,
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
      cache_ttl: default_cache_ttl(),
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

  pub fn build(&self) -> Result<Client> {
    let reqwest_client = self.to_builder().build()?;
    Ok(Client::new(
      self.cache_size.unwrap_or(0),
      self.cache_ttl,
      reqwest_client,
    ))
  }
}

pub struct Client {
  cache: ResponseCache,
  client: reqwest::Client,
}

impl Client {
  fn new(
    cache_size: usize,
    cache_ttl: Duration,
    client: reqwest::Client,
  ) -> Self {
    Self {
      cache: ResponseCache::new(cache_size, cache_ttl),
      client,
    }
  }

  pub async fn get(&self, url: &Url) -> Result<Response> {
    self.get_with(url, |req| req).await
  }

  pub async fn get_with(
    &self,
    url: &Url,
    f: impl FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
  ) -> Result<Response> {
    if let Some(resp) = self.cache.get_cached(url) {
      return Ok(resp);
    }

    let resp = f(self.client.get(url.clone())).send().await?;
    let resp = Response::from_reqwest_resp(resp).await?;
    self.cache.insert(url.clone(), resp.clone());
    Ok(resp)
  }
}

fn default_timeout() -> Duration {
  Duration::from_secs(10)
}

fn default_cache_ttl() -> Duration {
  Duration::from_secs(10 * 60)
}
