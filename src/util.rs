use serde::{Deserialize, Serialize};
use url::Url;

pub const USER_AGENT: &str =
  concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

// pub type DateTime = time::OffsetDateTime;
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
  #[error("Bad selector")]
  BadSelector(String),

  #[error("YAML parse error")]
  Yaml(#[from] serde_yaml::Error),

  #[error("Regex error")]
  Regex(#[from] regex::Error),

  #[error("{0}")]
  Message(String),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("IO error")]
  Io(#[from] std::io::Error),

  #[error("HTTP error")]
  Http(#[from] http::Error),

  #[error("Axum error")]
  Axum(#[from] axum::Error),

  #[error("RSS feed error")]
  Rss(#[from] rss::Error),

  #[error("Atom feed error")]
  Atom(#[from] atom_syndication::Error),

  #[error("Invalid URL {0}")]
  InvalidUrl(#[from] url::ParseError),

  #[error("Feed parsing error {0:?}")]
  FeedParse(&'static str),

  #[error("Feed merge error {0:?}")]
  FeedMerge(&'static str),

  #[error("Reqwest client error {0:?}")]
  Reqwest(#[from] reqwest::Error),

  #[error("HTTP status error {0:?} (url: {1})")]
  HttpStatus(reqwest::StatusCode, Url),

  #[error("Js execution error {0:?}")]
  Js(#[from] rquickjs::Error),

  #[error("Js exception {0}")]
  JsException(String),

  #[error("Failed to extract webpage {0:?}")]
  Readability(#[from] readability::error::Error),

  #[error("Config error {0:?}")]
  Config(#[from] ConfigError),

  #[error("{0}")]
  Message(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum SingleOrVec<T> {
  Single(T),
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
