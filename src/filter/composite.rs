use std::borrow::Cow;

use serde::Serialize;

use crate::{
  feed::{Feed, Post},
  util::Error,
};

use super::FeedFilter;

#[derive(Serialize)]
struct And {
  filters: Vec<Box<dyn FeedFilter>>,
}

#[derive(Serialize)]
struct Or {
  filters: Vec<Box<dyn FeedFilter>>,
}

#[derive(Serialize)]
struct Not {
  filter: Box<dyn FeedFilter>,
}

#[derive(Serialize)]
enum CompositeFilter {
  And(And),
  Or(Or),
  Not(Not),
}

#[derive(Serialize)]
enum StringOperand {
  Literal(String),
  PostField(String),
}

#[derive(Serialize)]
enum StringOperator {
  Equal,
  NotEqual,
  Contains,
  NotContains,
  StartsWith,
  EndsWith,
}

#[derive(Serialize)]
struct StringFilter {
  lhs: StringOperand,
  rhs: StringOperand,
  op: StringOperator,
}

impl FeedFilter for And {
  fn keep_post(&mut self, feed: &Feed, post: &Post) -> Result<bool, Error> {
    for filter in &mut self.filters {
      if !filter.keep_post(feed, post)? {
        return Ok(false);
      }
    }

    Ok(true)
  }
}

impl FeedFilter for Or {
  fn keep_post(&mut self, feed: &Feed, post: &Post) -> Result<bool, Error> {
    for filter in &mut self.filters {
      if filter.keep_post(feed, post)? {
        return Ok(true);
      }
    }

    Ok(false)
  }
}

impl FeedFilter for Not {
  fn keep_post(&mut self, feed: &Feed, post: &Post) -> Result<bool, Error> {
    Ok(!self.filter.keep_post(feed, post)?)
  }
}

impl FeedFilter for StringFilter {
  fn keep_post(&mut self, feed: &Feed, post: &Post) -> Result<bool, Error> {
    let lhs = match &self.lhs {
      StringOperand::Literal(s) => Cow::from(s),
      StringOperand::PostField(field) => post.get_field(&field)?,
    };

    let rhs = match &self.rhs {
      StringOperand::Literal(s) => Cow::from(s),
      StringOperand::PostField(field) => post.get_field(field)?,
    };

    let result = match self.op {
      StringOperator::Equal => lhs == rhs,
      StringOperator::NotEqual => lhs != rhs,
      StringOperator::Contains => lhs.contains(&*rhs),
      StringOperator::NotContains => !lhs.contains(&*rhs),
      StringOperator::StartsWith => lhs.starts_with(&*rhs),
      StringOperator::EndsWith => lhs.ends_with(&*rhs),
    };

    Ok(result)
  }
}

impl FeedFilter for CompositeFilter {
  fn keep_post(&mut self, feed: &Feed, post: &Post) -> Result<bool, Error> {
    match self {
      CompositeFilter::And(f) => f.keep_post(feed, post),
      CompositeFilter::Or(f) => f.keep_post(feed, post),
      CompositeFilter::Not(f) => f.keep_post(feed, post),
    }
  }
}
