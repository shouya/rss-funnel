use chrono::{DateTime, FixedOffset};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct FeedPreview {
  title: String,
  link: String,
  description: Option<String>,
  posts: Vec<PostPreview>,
}

#[derive(Debug, Serialize)]
pub struct PostPreview {
  title: String,
  author: Option<String>,
  link: String,
  body: String,
  published: DateTime<FixedOffset>,
}
