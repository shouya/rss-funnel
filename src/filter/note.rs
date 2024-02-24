use serde::{Deserialize, Serialize};

use super::{FeedFilterConfig, IdentityFilter};
use crate::util::Result;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct NoteFilterConfig {
  note: String,
}

#[async_trait::async_trait]
impl FeedFilterConfig for NoteFilterConfig {
  type Filter = IdentityFilter;

  async fn build(self) -> Result<Self::Filter> {
    Ok(IdentityFilter)
  }
}
