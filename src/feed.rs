use std::borrow::Cow;
use std::collections::HashMap;

use anyhow::anyhow;
use serde::Deserialize;
use serde::Serialize;
use time::format_description::well_known::Iso8601;

use crate::util::{DateTime, Result};

#[derive(Serialize, Deserialize)]
pub struct Feed {
  pub title: String,
  pub link: String,
  pub description: String,
  pub extra: HashMap<String, String>,
  pub posts: Vec<Post>,
}

#[derive(Serialize, Deserialize)]
pub struct Post {
  pub guid: String,
  pub title: String,
  pub description: String,
  pub authors: Vec<String>,
  pub link: String,
  pub extra: HashMap<String, String>,
  pub pub_date: DateTime,
}

impl Post {
  pub fn get_field(&self, field: &str) -> Result<Cow<str>> {
    match field {
      "guid" => Ok(Cow::from(&self.guid)),
      "title" => Ok(Cow::from(&self.title)),
      "description" => Ok(Cow::from(&self.description)),
      "link" => Ok(Cow::from(&self.link)),
      "pub_date" => self
        .pub_date
        .format(&Iso8601::DEFAULT)
        .map(|d| Cow::Owned(d.to_string()))
        .map_err(|e| e.into()),
      _ => self
        .extra
        .get(field)
        .map(|x| Cow::from(x))
        .ok_or_else(|| anyhow!("Post does not have field '{}'", field)),
    }
  }
}
