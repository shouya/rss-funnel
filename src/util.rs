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

  #[error("Feed error")]
  Rss(#[from] rss::Error),

  #[error("Invalid URL {0}")]
  InvalidUrl(#[from] url::ParseError),

  #[error("Feed parsing error {0:?}")]
  FeedParse(&'static str),

  #[error("Reqwest client error {0:?}")]
  Reqwest(#[from] reqwest::Error),

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
