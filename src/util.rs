use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

pub const USER_AGENT: &str =
  concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

lazy_static::lazy_static! {
  pub static ref DEMO_INSTANCE: Url =
    Url::parse("https://rss-funnel-demo.fly.dev/").unwrap();
}

pub fn is_env_set(name: &str) -> bool {
  let Ok(mut val) = std::env::var(name) else {
    return false;
  };

  val.make_ascii_lowercase();
  matches!(val.as_str(), "1" | "t" | "true" | "y" | "yes")
}

// pub type DateTime = time::OffsetDateTime;
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum JsError {
  #[error("{0}")]
  Message(String),

  #[error("Exception: {0}")]
  Exception(crate::js::Exception),

  #[error("{0}")]
  Error(#[from] rquickjs::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
  #[error("Bad selector: {0}")]
  BadSelector(String),

  #[error("YAML parse error: {0}")]
  Yaml(#[from] serde_yaml::Error),

  #[error("Regex error: {0}")]
  Regex(#[from] regex::Error),

  #[error("Invalid URL {0}")]
  InvalidUrl(#[from] url::ParseError),

  #[error("IO error: {0}")]
  Io(#[from] std::io::Error),

  #[error("Reqwest client error: {0}")]
  Reqwest(#[from] reqwest::Error),

  #[error("Js runtime initialization error: {0}")]
  Js(#[from] JsError),

  #[error("Client config error - bad header value: {0}")]
  ClientHeader(#[from] reqwest::header::InvalidHeaderValue),

  #[error("Duplicate endpoint: {0}")]
  DuplicateEndpoint(String),

  #[error("Feature {feature} not supported: {reason}")]
  FeatureNotSupported {
    feature: &'static str,
    reason: &'static str,
  },

  #[error("{0}")]
  Message(String),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("IO error: {0}")]
  Io(#[from] std::io::Error),

  #[error("HTTP error: {0}")]
  Http(#[from] http::Error),

  #[error("Axum error: {0}")]
  Axum(#[from] axum::Error),

  #[error("RSS feed error: {0}")]
  Rss(#[from] rss::Error),

  #[error("Atom feed error: {0}")]
  Atom(#[from] atom_syndication::Error),

  #[error("Invalid URL: {0}")]
  InvalidUrl(#[from] url::ParseError),

  #[error("Feed parsing error: {0}")]
  FeedParse(&'static str),

  #[error("Feed merge error: {0}")]
  FeedMerge(&'static str),

  #[error("Reqwest client error: {0}")]
  Reqwest(#[from] reqwest::Error),

  #[error("HTTP status error {0} (url: {1})")]
  HttpStatus(reqwest::StatusCode, Url),

  #[error("Js runtime error: {0}")]
  Js(#[from] JsError),

  #[error("Failed to extract webpage: {0}")]
  Readability(#[from] readability::error::Error),

  #[error("Config error: {0}")]
  Config(#[from] ConfigError),

  #[error("Tokio task join error: {0}")]
  Join(#[from] tokio::task::JoinError),

  #[error("Endpoint not found: {0}")]
  EndpointNotFound(String),

  #[error("Unsupported feed format: {0}")]
  UnsupportedFeedFormat(String),

  #[error("{0}")]
  Message(String),
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(untagged)]
pub enum SingleOrVec<T> {
  /// A single entry
  Single(T),
  /// A list of entries
  Vec(Vec<T>),
}

impl<T> Default for SingleOrVec<T> {
  fn default() -> Self {
    Self::empty()
  }
}

pub enum SingleOrVecIter<'a, T> {
  Single(std::iter::Once<&'a T>),
  Vec(std::slice::Iter<'a, T>),
}

impl<T> SingleOrVec<T> {
  pub fn empty() -> Self {
    Self::Vec(Vec::new())
  }

  pub fn into_vec(self) -> Vec<T> {
    match self {
      Self::Single(s) => vec![s],
      Self::Vec(v) => v,
    }
  }
}

impl<'a, T> IntoIterator for &'a SingleOrVec<T> {
  type Item = &'a T;
  type IntoIter = SingleOrVecIter<'a, T>;

  fn into_iter(self) -> Self::IntoIter {
    match self {
      SingleOrVec::Single(s) => SingleOrVecIter::Single(std::iter::once(s)),
      SingleOrVec::Vec(v) => SingleOrVecIter::Vec(v.iter()),
    }
  }
}

impl<'a, T> Iterator for SingleOrVecIter<'a, T> {
  type Item = &'a T;

  fn next(&mut self) -> Option<Self::Item> {
    match self {
      Self::Single(s) => s.next(),
      Self::Vec(v) => v.next(),
    }
  }
}
