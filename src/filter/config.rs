use serde::{Deserialize, Serialize};

use crate::util::Result;

use super::{js::JsConfig, FeedFilter, FeedFilterConfig};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FilterConfig {
  Js(JsConfig),
}

macro_rules! build_configs {
  ($self:ident; $($variant:ident),*) => {
    match $self {
      $(FilterConfig::$variant(config) => {
        let filter = config.build().await?;
        Ok(Box::new(filter) as Box<dyn FeedFilter>)
      })*
    }
  };

}

impl FilterConfig {
  pub async fn build(&self) -> Result<Box<dyn FeedFilter>> {
    build_configs!(self; Js)
  }
}
