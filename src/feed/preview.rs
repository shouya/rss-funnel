use chrono::{DateTime, FixedOffset};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct FeedPreview {
  pub title: String,
  pub link: String,
  pub description: Option<String>,
  pub posts: Vec<PostPreview>,
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
pub struct PostPreview {
  pub title: String,
  pub author: Option<String>,
  pub link: String,
  pub body: Option<String>,
  pub date: Option<DateTime<FixedOffset>>,
}
