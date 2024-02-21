use http::request::Parts;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
  client::Client,
  feed::Feed,
  util::{ConfigError, Error, Result},
};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum SourceConfig {
  Simple(String),
  FromScratch(BlankFeed),
}

#[derive(Clone, Debug)]
pub enum Source {
  AbsoluteUrl(Url),
  RelativeUrl(String),
  FromScratch(BlankFeed),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum FeedFormat {
  Rss,
  Atom,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BlankFeed {
  pub format: FeedFormat,
  pub title: String,
  pub link: Option<String>,
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
    }
  }
}

impl Source {
  pub async fn fetch_feed(
    &self,
    client: Option<&Client>,
    request: Option<&Parts>,
  ) -> Result<Feed> {
    if let Source::FromScratch(config) = self {
      let feed = Feed::from(config);
      return Ok(feed);
    }

    let client =
      client.ok_or_else(|| Error::Message("client not set".into()))?;
    let source_url = match self {
      Source::AbsoluteUrl(url) => url.clone(),
      Source::RelativeUrl(path) => {
        let request =
          request.ok_or_else(|| Error::Message("request not set".into()))?;
        let this_url: Url = request.uri.to_string().parse()?;
        this_url.join(path)?
      }
      Source::FromScratch(_) => unreachable!(),
    };

    client.fetch_feed(&source_url).await
  }
}
