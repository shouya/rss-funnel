use http::StatusCode;

pub use anyhow::Result;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ConfigError(#[from] pub anyhow::Error);

#[derive(Debug, thiserror::Error)]
pub enum JsError {
  #[error("{0}")]
  Message(String),

  #[error("JS exception: {0}")]
  Exception(crate::js::Exception),

  #[error(transparent)]
  Error(#[from] rquickjs::Error),
}

pub type JsResult<T> = std::result::Result<T, JsError>;

#[derive(Debug, thiserror::Error)]
#[error("Error in endpoint {0}")]
pub struct InEndpoint<T: AsRef<str>>(pub T);

#[derive(Debug, thiserror::Error)]
#[error("error in filter config for {1} ({0}th place)")]
pub struct InFilterConfig<T: AsRef<str>>(pub usize, pub T);

#[derive(Debug, thiserror::Error)]
#[error("error running {}th filter", self.0 + 1)]
pub struct InFilter(pub usize);

#[derive(Debug, thiserror::Error)]
#[error("error fetching source: {0}")]
pub struct InSource<T>(pub T);

// Marker types for HTTP status code mapping
#[derive(Debug, thiserror::Error)]
#[error("HTTP status error {0} (url: {1})")]
pub struct HttpStatusError(pub reqwest::StatusCode, pub url::Url);

#[derive(Debug, thiserror::Error)]
#[error(
  "source parameter {placeholder} failed to match validation: {validation} (input: {input})"
)]
pub struct SourceTemplateValidation {
  pub placeholder: String,
  pub validation: String,
  pub input: String,
}

#[derive(Debug, thiserror::Error)]
#[error("source template placeholder unspecified: {0}")]
pub struct MissingSourceTemplatePlaceholder(pub String);

#[derive(Debug, thiserror::Error)]
#[error("source URL unspecified for dynamic source")]
pub struct DynamicSourceUnspecified;

#[derive(Debug, thiserror::Error)]
#[error(
  "Can't infer app base, please refer to https://github.com/shouya/rss-funnel/wiki/App-base"
)]
pub struct BaseUrlNotInferred;

#[derive(Debug, thiserror::Error)]
#[error("Endpoint not found: {0}")]
pub struct EndpointNotFound(pub String);

pub fn into_http(e: anyhow::Error) -> (StatusCode, String) {
  for cause in e.chain() {
    if cause.downcast_ref::<SourceTemplateValidation>().is_some()
      || cause
        .downcast_ref::<MissingSourceTemplatePlaceholder>()
        .is_some()
    {
      return (StatusCode::BAD_REQUEST, format!("{e:?}"));
    }
  }

  (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}"))
}
