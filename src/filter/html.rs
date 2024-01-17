//! HTML related filters.
//!
//! # Included filters
//!
//! - [`RemoveElementConfig`] (`remove_element`): remove elements from HTML description
//! - [`KeepElementConfig`] (`keep_element`): keep only selected elements from HTML description
//! - [`SplitConfig`] (`split`): split a post into multiple posts

use ego_tree::NodeId;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

use crate::feed::Post;
use crate::util::{Error, Result};
use crate::{feed::Feed, util::ConfigError};

use super::{FeedFilter, FeedFilterConfig};

/// Remove elements from HTML description.
///
/// You can specify the list of CSS `selectors` to remove.
///
/// # Example
///
/// ```yaml
/// filters:
///   - remove_element:
///       - img[src$=".gif"]
///       - span.ads
/// ```
#[doc(alias = "remove_element")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct RemoveElementConfig {
  selectors: Vec<String>,
}

pub struct RemoveElement {
  selectors: Vec<Selector>,
}

fn parse_selector(selector: &str) -> Result<Selector> {
  Selector::parse(selector)
    .map_err(|e| ConfigError::BadSelector(format!("{}: {}", selector, e)))
    .map_err(|e| e.into())
}

#[async_trait::async_trait]
impl FeedFilterConfig for RemoveElementConfig {
  type Filter = RemoveElement;

  async fn build(&self) -> Result<Self::Filter> {
    let mut selectors = vec![];
    for selector in &self.selectors {
      let parsed = parse_selector(selector)?;

      selectors.push(parsed);
    }

    Ok(RemoveElement { selectors })
  }
}

impl RemoveElement {
  fn filter_description(&self, description: &str) -> Option<String> {
    let mut html = Html::parse_fragment(description);
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
    let mut posts = feed.take_posts();

    for post in &mut posts {
      let description_mut = post.description_or_insert();
      if let Some(description) = self.filter_description(description_mut) {
        *description_mut = description;
      }
    }

    feed.set_posts(posts);
    Ok(())
  }
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeepElementConfig {
  selector: String,
}

pub struct KeepElement {
  selectors: Vec<Selector>,
}

#[async_trait::async_trait]
impl FeedFilterConfig for KeepElementConfig {
  type Filter = KeepElement;

  // TODO: decide whether we want to support iteratively narrowed
  // selector. Multiple selectors here may create more confusion than
  // being useful.
  async fn build(&self) -> Result<Self::Filter> {
    let selectors = vec![parse_selector(&self.selector)?];
    Ok(KeepElement { selectors })
  }
}

impl KeepElement {
  fn keep_only_selected(html: &mut Html, selected: &[NodeId]) -> Option<()> {
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

  pub fn filter_description(&self, description: &str) -> Option<String> {
    let mut html = Html::parse_fragment(description);

    for selector in &self.selectors {
      let mut selected = vec![];
      for elem in html.select(selector) {
        selected.push(elem.id());
      }

      if Self::keep_only_selected(&mut html, &selected).is_none() {
        return Some("<no element kept>".to_string());
      }
    }

    Some(html.html())
  }
}

#[async_trait::async_trait]
impl FeedFilter for KeepElement {
  async fn run(&self, feed: &mut Feed) -> Result<()> {
    let mut posts = feed.take_posts();

    for post in &mut posts {
      let description_mut = post.description_or_insert();
      if let Some(description) = self.filter_description(description_mut) {
        *description_mut = description;
      }
    }

    feed.set_posts(posts);
    Ok(())
  }
}

#[derive(Serialize, Deserialize)]
pub struct SplitConfig {
  title_selector: String,
  link_selector: Option<String>,
  description_selector: Option<String>,
  author_selector: Option<String>,
}

pub struct Split {
  title_selector: Selector,
  link_selector: Option<Selector>,
  description_selector: Option<Selector>,
  author_selector: Option<Selector>,
}

#[async_trait::async_trait]
impl FeedFilterConfig for SplitConfig {
  type Filter = Split;

  async fn build(&self) -> Result<Self::Filter> {
    let parse_selector_opt = |s: &Option<String>| -> Result<Option<Selector>> {
      match s {
        Some(s) => Ok(Some(parse_selector(s)?)),
        None => Ok(None),
      }
    };

    let title_selector = parse_selector(&self.title_selector)?;
    let link_selector = parse_selector_opt(&self.link_selector)?;
    let description_selector = parse_selector_opt(&self.description_selector)?;
    let author_selector = parse_selector_opt(&self.author_selector)?;

    Ok(Split {
      title_selector,
      link_selector,
      description_selector,
      author_selector,
    })
  }
}

impl Split {
  fn select_title(&self, doc: &Html) -> Result<Vec<String>> {
    Ok(
      doc
        .select(&self.title_selector)
        .map(|e| e.text().collect())
        .collect(),
    )
  }

  fn expand_link(base_link: &str, link: &str) -> String {
    if link.starts_with("http://") || link.starts_with("https://") {
      return link.to_string();
    }

    let mut base_link = base_link.to_string();
    if let Some(i) = base_link.rfind('/') {
      base_link.truncate(i + 1);
    }
    base_link.push_str(link);

    base_link
  }

  fn select_link(&self, base_link: &str, doc: &Html) -> Result<Vec<String>> {
    let link_selector =
      self.link_selector.as_ref().unwrap_or(&self.title_selector);

    let links = doc
      .select(link_selector)
      .map(|e| {
        e.value()
          .attr("href")
          .map(|s| s.to_string())
          .map(|link| Self::expand_link(base_link, &link))
          .ok_or_else(|| {
            Error::Message("Selector error: link has no href".into())
          })
      })
      .collect::<Result<Vec<_>>>()?;

    Ok(links)
  }

  fn select_description(&self, doc: &Html) -> Result<Option<Vec<String>>> {
    let Some(description_selector) = &self.description_selector else {
      return Ok(None);
    };

    let descriptions =
      doc.select(description_selector).map(|e| e.html()).collect();

    Ok(Some(descriptions))
  }

  fn select_author(&self, doc: &Html) -> Result<Option<Vec<String>>> {
    if self.author_selector.is_none() {
      return Ok(None);
    }

    let authors = doc
      .select(self.author_selector.as_ref().unwrap())
      .map(|e| e.text().collect())
      .collect();

    Ok(Some(authors))
  }

  fn prepare_template(&self, post: &Post) -> Post {
    let mut template_post = post.clone();
    if let Some(description) = template_post.description_mut() {
      description.clear()
    }

    if self.author_selector.is_some() {
      if let Some(author) = template_post.author_mut() {
        author.clear();
      }
    }
    template_post
  }

  fn apply_template(
    &self,
    template: &mut Post,
    title: &str,
    link: &str,
    description: Option<&str>,
    author: Option<&str>,
  ) {
    template.set_title(title);
    template.set_link(link);
    if let Some(description) = description {
      template.set_description(description);
    }
    if let Some(author) = author {
      template.set_author(author);
    }
    template.set_guid(link);
  }

  fn split(&self, post: &Post) -> Result<Vec<Post>> {
    let mut posts = vec![];

    let doc = Html::parse_fragment(post.description_or_err()?);

    let titles = self.select_title(&doc)?;
    let links = self.select_link(post.link_or_err()?, &doc)?;
    if titles.len() != links.len() {
      let msg = format!(
        "Selector error: title ({}) and link ({}) count mismatch",
        titles.len(),
        links.len()
      );
      return Err(Error::Message(msg));
    }

    let n = titles.len();
    let descriptions = transpose_option_vec(self.select_description(&doc)?, n);
    let authors = transpose_option_vec(self.select_author(&doc)?, n);

    if titles.len() != descriptions.len() || titles.len() != authors.len() {
      let msg = format!(
        "Selector error: title ({}), link ({}), \
         description ({}), and author ({}) count mismatch",
        titles.len(),
        links.len(),
        descriptions.len(),
        authors.len()
      );
      return Err(Error::Message(msg));
    }

    let iter = itertools::multizip((titles, links, descriptions, authors));

    for (title, link, description, author) in iter {
      let mut post = self.prepare_template(post);
      self.apply_template(
        &mut post,
        &title,
        &link,
        description.as_deref(),
        author.as_deref(),
      );
      posts.push(post);
    }

    Ok(posts)
  }
}

fn transpose_option_vec<T: Clone>(
  v: Option<Vec<T>>,
  n: usize,
) -> Vec<Option<T>> {
  match v {
    Some(v) => v.into_iter().map(|x| Some(x)).collect(),
    None => vec![None; n],
  }
}

#[async_trait::async_trait]
impl FeedFilter for Split {
  async fn run(&self, feed: &mut Feed) -> Result<()> {
    let mut posts = vec![];
    for post in &feed.take_posts() {
      let mut split_posts = self.split(post)?;
      posts.append(&mut split_posts);
    }

    feed.set_posts(posts);
    Ok(())
  }
}
