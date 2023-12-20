pub const USER_AGENT: &str =
  concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

// pub type DateTime = time::OffsetDateTime;
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("Get non-2xx response from upstream")]
  UpstreamNon2xx(http::Response<axum::body::Body>),

  #[error("IO error")]
  Io(#[from] std::io::Error),

  #[error("HTTP error")]
  Http(#[from] http::Error),

  #[error("Hyper client error: {0:?}")]
  HyperClient(#[from] hyper_util::client::legacy::Error),

  #[error("Axum error")]
  Axum(#[from] axum::Error),

  #[error("YAML parse error")]
  Yaml(#[from] serde_yaml::Error),

  #[error("Bad time format")]
  TimeFormat(#[from] time::error::Format),

  #[error("Feed error")]
  Rss(#[from] rss::Error),

  #[error("Feed parsing error {0:?}")]
  FeedParse(&'static str),

  #[error("Reqwest client error {0:?}")]
  Reqwest(#[from] reqwest::Error),

  #[error("Js execution error {0:?}")]
  Js(#[from] rquickjs::Error),

  #[error("Js exception {0}")]
  JsException(String),

  #[error("{0}")]
  Message(String),

  #[error("Generic anyhow error")]
  Generic(#[from] anyhow::Error),
}
