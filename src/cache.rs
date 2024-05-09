use std::{
  hash::Hash,
  num::NonZeroUsize,
  sync::RwLock,
  time::{Duration, Instant},
};

use lru::LruCache;

pub struct Timed<T> {
  value: T,
  created: Instant,
}

pub struct TimedLruCache<K: Hash + Eq, V: Clone> {
  map: RwLock<LruCache<K, Timed<V>>>,
  timeout: Duration,
}

impl<K: Hash + Eq, V: Clone> TimedLruCache<K, V> {
  pub fn new(max_entries: usize, timeout: Duration) -> Self {
    let max_entries = max_entries.try_into().unwrap_or(NonZeroUsize::MIN);
    Self {
      map: RwLock::new(LruCache::new(max_entries)),
      timeout,
    }
  }

  pub fn get_cached(&self, key: &K) -> Option<V> {
    let mut map = self.map.write().ok()?;
    let entry = map.get(key)?;
    if entry.created.elapsed() > self.timeout {
      map.pop(key);
      return None;
    }
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
}
