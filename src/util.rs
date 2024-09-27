use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

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
