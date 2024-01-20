use std::time::Duration;

use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::util::Result;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientConfig {
  user_agent: Option<String>,
  accept: Option<String>,
  set_cookie: Option<String>,
  referer: Option<String>,
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

  pub fn build(&self) -> Result<reqwest::Client> {
    Ok(self.to_builder().build()?)
  }
}

fn default_timeout() -> Duration {
  Duration::from_secs(10)
}
