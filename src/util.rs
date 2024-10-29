mod html;

use url::Url;

pub use self::html::{convert_relative_url, fragment_root_node_id, html_body};

pub const USER_AGENT: &str =
  concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

lazy_static::lazy_static! {
  pub static ref DEMO_INSTANCE: Url =
    Url::parse("https://rss-funnel-demo.fly.dev/").unwrap();
}

#[allow(unused)]
pub fn is_env_set(name: &str) -> bool {
  let Ok(mut val) = std::env::var(name) else {
    return false;
  };

  val.make_ascii_lowercase();
  matches!(val.as_str(), "1" | "t" | "true" | "y" | "yes")
}

mod path_prefix {
  use std::sync::LazyLock;

  const DEFAULT_PATH_PREFIX: &str = "/";
  pub static PATH_PREFIX: LazyLock<Box<str>> = LazyLock::new(|| {
    let prefix = std::env::var("RSS_FUNNEL_PATH_PREFIX")
      .ok()
      .or_else(|| {
        super::app_base_from_env()
          .as_ref()
          .map(|url| url.path().to_owned())
      })
      .unwrap_or_else(|| DEFAULT_PATH_PREFIX.to_owned())
      .into_boxed_str();
    assert!(prefix.ends_with("/"));
    prefix
  });

  pub fn relative_path(path: &str) -> String {
    debug_assert!(!path.starts_with("/"));
    format!("{}{path}", *PATH_PREFIX)
  }
}

pub use self::path_prefix::relative_path;

mod app_base {
  use std::sync::LazyLock;
  use url::Url;

  static APP_BASE_URL: LazyLock<Option<Url>> = LazyLock::new(|| {
    let var = std::env::var("RSS_FUNNEL_APP_BASE").ok();
    var.map(|v| {
      v.parse()
        .expect("Invalid base url specified in RSS_FUNNEL_APP_BASE")
    })
  });

  pub fn app_base_from_env() -> &'static Option<Url> {
    &APP_BASE_URL
  }
}

pub use self::app_base::app_base_from_env;

mod single_or_vec {
  use schemars::JsonSchema;
  use serde::{Deserialize, Serialize};

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
}

pub use self::single_or_vec::SingleOrVec;

mod cache {
  use std::{
    hash::Hash,
    num::NonZeroUsize,
    sync::{
      atomic::{AtomicUsize, Ordering},
      RwLock,
    },
    time::{Duration, Instant},
  };

  use lru::LruCache;

  struct Timed<T> {
    value: T,
    created: Instant,
  }

  pub struct TimedLruCache<K: Hash + Eq, V: Clone> {
    map: RwLock<LruCache<K, Timed<V>>>,
    misses: AtomicUsize,
    hits: AtomicUsize,
    timeout: Duration,
  }

  impl<K: Hash + Eq, V: Clone> TimedLruCache<K, V> {
    pub fn new(max_entries: usize, timeout: Duration) -> Self {
      let max_entries = max_entries.try_into().unwrap_or(NonZeroUsize::MIN);
      Self {
        map: RwLock::new(LruCache::new(max_entries)),
        timeout,
        misses: AtomicUsize::new(0),
        hits: AtomicUsize::new(0),
      }
    }

    pub fn get_cached(&self, key: &K) -> Option<V> {
      let mut map = self.map.write().ok()?;
      let Some(entry) = map.get(key) else {
        self.misses.fetch_add(1, Ordering::Relaxed);
        return None;
      };

      if entry.created.elapsed() > self.timeout {
        self.misses.fetch_add(1, Ordering::Relaxed);
        map.pop(key);
        return None;
      }

      self.hits.fetch_add(1, Ordering::Relaxed);
      Some(entry.value.clone())
    }

    pub fn insert(&self, key: K, value: V) -> Option<()> {
      let timed = Timed {
        value,
        created: Instant::now(),
      };
      self.map.write().ok()?.push(key, timed);
      Some(())
    }

    // hit, miss
    #[allow(unused)]
    pub fn stats(&self) -> (usize, usize) {
      (
        self.hits.load(Ordering::Relaxed),
        self.misses.load(Ordering::Relaxed),
      )
    }
  }
}

pub use self::cache::TimedLruCache;
