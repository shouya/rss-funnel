use std::{
  num::NonZeroUsize,
  sync::{Arc, RwLock},
  time::{Duration, Instant},
};

use lru::LruCache;
use reqwest::header::HeaderMap;
use url::Url;

use crate::util::Result;

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
  status: reqwest::StatusCode,
  headers: HeaderMap,
  body: Box<[u8]>,
}

impl Response {
  pub async fn from_reqwest_resp(resp: reqwest::Response) -> Result<Self> {
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.bytes().await?.to_vec().into_boxed_slice();

    Ok(Self {
      inner: Arc::new(InnerResponse {
        status,
        headers,
        body,
      }),
    })
  }
}
