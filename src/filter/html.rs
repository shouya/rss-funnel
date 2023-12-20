use ego_tree::NodeId;
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

#[derive(Serialize, Deserialize)]
pub struct KeepElementConfig {
  selector: String,
}

pub struct KeepElement {
  selectors: Vec<scraper::Selector>,
}

#[async_trait::async_trait]
impl FeedFilterConfig for KeepElementConfig {
  type Filter = KeepElement;

  // TODO: decide whether we want to support iteratively narrowed
  // selector. Multiple selectors here may create more confusion than
  // being useful.
  async fn build(&self) -> Result<Self::Filter> {
    let mut selectors = vec![];
    for selector in [&self.selector] {
      let parsed = scraper::Selector::parse(selector).map_err(|err| {
        ConfigError::BadSelector(format!("{}: {}", selector, err))
      })?;

      selectors.push(parsed);
    }

    Ok(KeepElement { selectors })
  }
}

impl KeepElement {
  fn keep_only_selected(
    html: &mut scraper::Html,
    selected: &[NodeId],
  ) -> Option<()> {
    let tree = &mut html.tree;

    if selected.is_empty() {
      return None;
    }

    // remove all children of root to make the selected nodes the only children
    while let Some(mut child) = tree.root_mut().first_child() {
      child.detach();
    }
    for node_id in selected {
      tree.root_mut().append_id(*node_id);
    }

    Some(())
  }

  fn filter_content(&self, content: &str) -> Option<String> {
    let mut html = scraper::Html::parse_fragment(content);

    for selector in &self.selectors {
      let mut selected = vec![];
      for elem in html.select(selector) {
        selected.push(elem.id());
      }

      if let None = Self::keep_only_selected(&mut html, &selected) {
        return Some("<no element kept>".to_string());
      }
    }

    Some(html.html())
  }
}

#[async_trait::async_trait]
impl FeedFilter for KeepElement {
  async fn run(&self, feed: &mut Feed) -> Result<()> {
    for post in &mut feed.posts {
      if let Some(content) = self.filter_content(&post.description) {
        post.description = content;
      }
    }

    Ok(())
  }
}
