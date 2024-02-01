use std::{
  num::NonZeroUsize,
  sync::{Arc, RwLock},
  time::{Duration, Instant},
};

use lru::LruCache;
use mime::Mime;
use reqwest::header::HeaderMap;
use url::Url;

use crate::util::{Error, Result};

struct Timed<T> {
  value: T,
  created: Instant,
}

pub struct ResponseCache {
  map: RwLock<LruCache<Url, Timed<Response>>>,
  timeout: Duration,
}

impl ResponseCache {
  pub fn new(max_entries: usize, timeout: Duration) -> Self {
    let max_entries = max_entries.try_into().unwrap_or(NonZeroUsize::MIN);
    Self {
      map: RwLock::new(LruCache::new(max_entries)),
      timeout,
    }
  }

  pub fn get_cached(&self, url: &Url) -> Option<Response> {
    let mut map = self.map.write().ok()?;
    let Some(entry) = map.get(url) else {
      return None;
    };
    if entry.created.elapsed() > self.timeout {
      map.pop(url);
      return None;
    }
    Some(entry.value.clone())
  }

  pub fn insert(&self, url: Url, response: Response) -> Option<()> {
    let timed = Timed {
      value: response,
      created: Instant::now(),
    };
    self.map.write().ok()?.push(url, timed);
    Some(())
  }
}

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

    let full = &self.inner.body;
    let (text, _, _) = encoding.decode(full);
    Ok(text.into_owned())
  }

  pub fn text(&self) -> Result<String> {
    self.text_with_charset("utf-8")
  }

  pub fn content_type(&self) -> Option<Mime> {
    self.header("content-type").and_then(|v| v.parse().ok())
  }
}
