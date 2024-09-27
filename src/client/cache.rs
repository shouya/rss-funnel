use std::sync::Arc;

use mime::Mime;
use reqwest::header::HeaderMap;
use url::Url;

use crate::{util::TimedLruCache, Error, Result};

pub type ResponseCache = TimedLruCache<Url, Response>;

#[derive(Clone)]
pub struct Response {
  inner: Arc<InnerResponse>,
}

struct InnerResponse {
  url: Url,
  status: reqwest::StatusCode,
  headers: HeaderMap,
  body: Box<[u8]>,
}

impl Response {
  pub async fn from_reqwest_resp(resp: reqwest::Response) -> Result<Self> {
    let status = resp.status();
    let headers = resp.headers().clone();
    let url = resp.url().clone();
    let body = resp.bytes().await?.to_vec().into_boxed_slice();
    let resp = InnerResponse {
      url,
      status,
      headers,
      body,
    };

    Ok(Self {
      inner: Arc::new(resp),
    })
  }

  #[cfg(test)]
  pub fn new(
    url: Url,
    status: reqwest::StatusCode,
    headers: HeaderMap,
    body: Box<[u8]>,
  ) -> Self {
    Self {
      inner: Arc::new(InnerResponse {
        url,
        status,
        headers,
        body,
      }),
    }
  }

  #[cfg(test)]
  pub(super) fn from_fixture(url: &Url) -> Self {
    use std::path::PathBuf;

    let path: PathBuf =
      format!("{}/fixtures/{}", env!("CARGO_MANIFEST_DIR"), url.path()).into();
    let content_type = url
      .query_pairs()
      .find(|(k, _)| k == "content_type")
      .map(|(_, v)| v.to_string())
      .unwrap_or_else(|| "text/xml; charset=utf-8".into());

    if !path.exists() {
      panic!("fixture file does not exist: {}", path.display());
    }

    let mut headers = HeaderMap::new();
    headers.insert(
      "content-type",
      content_type.parse().expect("invalid content-type"),
    );
    let body = std::fs::read(path)
      .expect("failed to read fixture file")
      .into_boxed_slice();

    Self {
      inner: Arc::new(InnerResponse {
        url: url.clone(),
        status: reqwest::StatusCode::OK,
        headers,
        body,
      }),
    }
  }

  pub fn error_for_status(self) -> Result<Self> {
    let status = self.inner.status;
    if status.is_client_error() || status.is_server_error() {
      return Err(Error::HttpStatus(status, self.inner.url.clone()));
    }

    Ok(self)
  }

  pub fn header(&self, name: &str) -> Option<&str> {
    self.inner.headers.get(name).and_then(|v| v.to_str().ok())
  }

  pub fn text_with_charset(&self, default_encoding: &str) -> Result<String> {
    let content_type = self.content_type();
    let encoding_name = content_type
      .as_ref()
      .and_then(|mime| {
        mime.get_param("charset").map(|charset| charset.as_str())
      })
      .unwrap_or(default_encoding);
    let encoding = encoding_rs::Encoding::for_label(encoding_name.as_bytes())
      .unwrap_or(encoding_rs::UTF_8);

    let (text, _, _) = encoding.decode(self.body());
    Ok(text.into_owned())
  }

  pub fn text(&self) -> Result<String> {
    self.text_with_charset("utf-8")
  }

  pub fn content_type(&self) -> Option<Mime> {
    self.header("content-type").and_then(|v| v.parse().ok())
  }

  // can only be called the first time the response is constructed
  pub(super) fn set_content_type(&mut self, content_type: &str) {
    let inner = Arc::get_mut(&mut self.inner).expect("response is shared");

    inner.headers.insert(
      "content-type",
      content_type.parse().expect("invalid content_type"),
    );
  }

  #[allow(dead_code)]
  pub fn url(&self) -> &Url {
    &self.inner.url
  }
  #[allow(dead_code)]
  pub fn status(&self) -> reqwest::StatusCode {
    self.inner.status
  }
  #[allow(dead_code)]
  pub fn headers(&self) -> &HeaderMap {
    &self.inner.headers
  }

  pub fn body(&self) -> &[u8] {
    &self.inner.body
  }
}
