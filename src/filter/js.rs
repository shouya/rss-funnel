use serde::{Deserialize, Serialize};

use crate::feed::Feed;
use crate::util::Result;

use super::{FeedFilter, FeedFilterConfig};

#[derive(Serialize, Deserialize)]
pub struct JsConfig {
  /// The javascript code to run
  code: String,
}

pub struct JsFilter {
  code: String,
}

#[async_trait::async_trait]
impl FeedFilterConfig for JsConfig {
  type Filter = JsFilter;

  async fn build(&self) -> Result<Self::Filter> {
    Ok(Self::Filter {
      code: self.code.clone(),
    })
  }
}

#[async_trait::async_trait]
impl FeedFilter for JsFilter {
  async fn run(&mut self, _feed: &mut Feed) -> Result<()> {
    Ok(())
  }
}
