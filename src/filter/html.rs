use serde::{Deserialize, Serialize};

use crate::util::Result;
use crate::{feed::Feed, util::ConfigError};

use super::{FeedFilter, FeedFilterConfig};

#[derive(Serialize, Deserialize)]
pub struct RemoveElementConfig {
  selectors: Vec<String>,
}

pub struct RemoveElement {
  selectors: Vec<scraper::Selector>,
}

#[async_trait::async_trait]
impl FeedFilterConfig for RemoveElementConfig {
  type Filter = RemoveElement;

  async fn build(&self) -> Result<Self::Filter> {
    let mut selectors = vec![];
    for selector in &self.selectors {
      let parsed = scraper::Selector::parse(selector).map_err(|err| {
        ConfigError::BadSelector(format!("{}: {}", selector, err))
      })?;

      selectors.push(parsed);
    }

    Ok(RemoveElement { selectors })
  }
}

impl RemoveElement {
  fn filter_content(&self, content: &str) -> Option<String> {
    let mut html = scraper::Html::parse_fragment(content);
    let mut selected_node_ids = vec![];
    for selector in &self.selectors {
      for elem in html.select(selector) {
        selected_node_ids.push(elem.id());
      }
    }

    for id in selected_node_ids {
      if let Some(mut node) = html.tree.get_mut(id) {
        node.detach();
      }
    }

    Some(html.html())
  }
}

#[async_trait::async_trait]
impl FeedFilter for RemoveElement {
  async fn run(&self, feed: &mut Feed) -> Result<()> {
    for post in &mut feed.posts {
      if let Some(content) = self.filter_content(&post.description) {
        post.description = content;
      }
    }

    Ok(())
  }
}
