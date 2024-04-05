use chrono::{DateTime, FixedOffset};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct FeedPreview {
  pub title: String,
  pub link: String,
  pub description: Option<String>,
  pub posts: Vec<PostPreview>,
}

#[derive(Debug, Serialize)]
pub struct PostPreview {
  pub title: String,
  pub author: Option<String>,
  pub link: String,
  pub body: Option<String>,
  pub published: Option<DateTime<FixedOffset>>,
}
