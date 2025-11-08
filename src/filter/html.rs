//! HTML related filters.
//!
//! # Included filters
//!
//! - [`RemoveElementConfig`] (`remove_element`): remove elements from HTML body
//! - [`KeepElementConfig`] (`keep_element`): keep only selected elements from HTML body
//! - [`SplitConfig`] (`split`): split a post into multiple posts

use chrono::{DateTime, FixedOffset};
use ego_tree::NodeId;
use schemars::JsonSchema;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};

use crate::feed::Post;
use crate::{ConfigError, feed::Feed};
use crate::{Error, Result};

use super::{FeedFilter, FeedFilterConfig, FilterContext};

/// Remove elements from the post's body parsed as HTML. Specify the list of CSS
/// selectors for elements to remove.<br><br>
///
///   - `remove_element`:<br>
///       - img[src$=".gif"]<br>
///       - span.ads
#[doc(alias = "remove_element")]
#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
pub struct RemoveElementConfig {
  selectors: Vec<String>,
}

pub struct RemoveElement {
  selectors: Vec<Selector>,
}

// can't define FromStr for Selector due to Rust's orphan rule
fn parse_selector(selector: &str) -> Result<Selector, ConfigError> {
  Selector::parse(selector)
    .map_err(|e| ConfigError::BadSelector(format!("{selector}: {e}")))
}

#[async_trait::async_trait]
impl FeedFilterConfig for RemoveElementConfig {
  type Filter = RemoveElement;

  async fn build(self) -> Result<Self::Filter, ConfigError> {
    let mut selectors = vec![];
    for selector in self.selectors {
      let parsed = parse_selector(&selector)?;

      selectors.push(parsed);
    }

    Ok(RemoveElement { selectors })
  }
}

impl RemoveElement {
  fn filter_body(&self, body: &mut String) {
    let mut html = Html::parse_fragment(body);
    let mut selected_node_ids = vec![];
    for selector in &self.selectors {
      for elem in html.select(selector) {
        selected_node_ids.push(elem.id());
      }
    }

    if selected_node_ids.is_empty() {
      return;
    }

    for id in selected_node_ids {
      if let Some(mut node) = html.tree.get_mut(id) {
        node.detach();
      }
    }

    body.replace_range(.., &html.html());
  }
}

#[async_trait::async_trait]
impl FeedFilter for RemoveElement {
  async fn run(
    &self,
    _ctx: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    let mut posts = feed.take_posts();

    for post in &mut posts {
      post.modify_bodies(|body| {
        self.filter_body(body);
      });
    }

    feed.set_posts(posts);
    Ok(feed)
  }

  fn cache_granularity(&self) -> super::CacheGranularity {
    super::CacheGranularity::FeedAndPost
  }
}

/// Keep only selected elements from the post's body parsed as HTML.
///
/// You can specify the a CSS selector to keep a specific element.
///
/// # Example
///
/// ```yaml
///   - keep_element: img[src$=".gif"]
/// ```
#[derive(
  JsonSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash,
)]
#[serde(transparent)]
pub struct KeepElementConfig {
  selector: String,
}

#[derive(Clone)]
pub struct KeepElement {
  selectors: Vec<Selector>,
}

#[async_trait::async_trait]
impl FeedFilterConfig for KeepElementConfig {
  type Filter = KeepElement;

  // TODO: decide whether we want to support iteratively narrowed
  // selector. Multiple selectors here may create more confusion than
  // being useful.
  async fn build(self) -> Result<Self::Filter, ConfigError> {
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

  pub fn filter_body(&self, body: &mut String) {
    let mut html = Html::parse_fragment(body);

    for selector in &self.selectors {
      let mut selected = vec![];
      for elem in html.select(selector) {
        selected.push(elem.id());
      }

      if Self::keep_only_selected(&mut html, &selected).is_none() {
        body.replace_range(.., "<no element kept>");
        return;
      }
    }

    body.replace_range(.., &html.html());
  }
}

#[async_trait::async_trait]
impl FeedFilter for KeepElement {
  async fn run(
    &self,
    _ctx: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    let mut posts = feed.take_posts();

    for post in &mut posts {
      post.modify_bodies(|body| self.filter_body(body));
    }

    feed.set_posts(posts);
    Ok(feed)
  }

  fn cache_granularity(&self) -> super::CacheGranularity {
    super::CacheGranularity::FeedAndPost
  }
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct SplitConfig {
  /// The CSS selector for the title elements. The textContent of the
  /// selected elements will be used.
  title_selector: String,
  /// The CSS selector for the &lt;a&gt element. The "href" attribute
  /// of the selected elements will be used. Defaults to the same as
  /// `title_selector`. If specified, it must select the same number of
  /// elements as `title_selector`.
  link_selector: Option<String>,
  /// The CSS selector for the body elements. The innerHTML of
  /// the selected elements will be used. If specified, it must select
  /// the same number of elements as `title_selector`.
  #[deprecated(note = "Use `body_selector` instead")]
  description_selector: Option<String>,
  /// The CSS selector for the body elements. The innerHTML of
  /// the selected elements will be used. If specified, it must select
  /// the same number of elements as `title_selector`.
  body_selector: Option<String>,
  /// The CSS selector for the author elements. The textContent of the
  /// selected elements will be used. If specified, it must select the
  /// same number of elements as `title_selector`.
  author_selector: Option<String>,
  /// The CSS selector for the elements with the publication date.
  /// rss-funnel uses heuristics to find the publication date from the
  /// textContent and the attributes of the selected elements.
  date_selector: Option<String>,
}

pub struct Split {
  title_selector: Selector,
  link_selector: Option<Selector>,
  body_selector: Option<Selector>,
  author_selector: Option<Selector>,
  date_selector: Option<Selector>,
}

#[async_trait::async_trait]
impl FeedFilterConfig for SplitConfig {
  type Filter = Split;

  async fn build(self) -> Result<Self::Filter, ConfigError> {
    let parse_selector_opt =
      |s: &Option<String>| -> Result<Option<Selector>, ConfigError> {
        match s {
          Some(s) => Ok(Some(parse_selector(s)?)),
          None => Ok(None),
        }
      };

    let title_selector = parse_selector(&self.title_selector)?;
    let link_selector = parse_selector_opt(&self.link_selector)?;
    #[allow(deprecated)]
    let body_selector = parse_selector_opt(&self.body_selector)
      .or_else(|_| parse_selector_opt(&self.description_selector))?;
    let author_selector = parse_selector_opt(&self.author_selector)?;
    let date_selector = parse_selector_opt(&self.date_selector)?;

    Ok(Split {
      title_selector,
      link_selector,
      body_selector,
      author_selector,
      date_selector,
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
          .map(std::string::ToString::to_string)
          .map(|link| Self::expand_link(base_link, &link))
          .ok_or_else(|| {
            Error::Message("Selector error: link has no href".into())
          })
      })
      .collect::<Result<Vec<_>>>()?;

    Ok(links)
  }

  fn select_body(&self, doc: &Html) -> Result<Option<Vec<String>>> {
    let Some(body_selector) = &self.body_selector else {
      return Ok(None);
    };

    let bodies = doc.select(body_selector).map(|e| e.html()).collect();

    Ok(Some(bodies))
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

  fn select_date(
    &self,
    doc: &Html,
  ) -> Result<Option<Vec<DateTime<FixedOffset>>>> {
    let Some(date_selector) = self.date_selector.as_ref() else {
      return Ok(None);
    };

    let dates = doc
      .select(date_selector)
      .map(parse_date_from_element)
      .collect::<Option<Vec<_>>>();

    Ok(dates)
  }

  fn prepare_template(&self, post: &Post) -> Post {
    let mut template_post = post.clone();
    template_post.modify_bodies(std::string::String::clear);

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
    body: Option<&str>,
    author: Option<&str>,
    pub_date: Option<DateTime<FixedOffset>>,
  ) {
    template.set_title(title);
    template.set_link(link);
    if let Some(new_body) = body {
      template.modify_bodies(|body| body.replace_range(.., new_body));
    }
    if let Some(author) = author {
      template.set_author(author);
    }
    if let Some(pub_date) = pub_date {
      template.set_pub_date(pub_date);
    }
    template.set_guid(link);
  }

  fn split(&self, mut post: Post) -> Result<Vec<Post>> {
    let mut posts = vec![];

    let Some(body) = post.first_body() else {
      let body = post.create_body();
      body.push_str("split failed: no body");
      return Ok(vec![post]);
    };

    let doc = Html::parse_fragment(body);

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
    let bodies = transpose_option_vec(self.select_body(&doc)?, n);
    let authors = transpose_option_vec(self.select_author(&doc)?, n);
    let pub_dates = transpose_option_vec(self.select_date(&doc)?, n);

    if titles.len() != bodies.len()
      || titles.len() != authors.len()
      || titles.len() != pub_dates.len()
    {
      let msg = format!(
        "Selector error: title ({}), link ({}), body ({}), author ({}), and date ({}) count mismatch",
        titles.len(),
        links.len(),
        bodies.len(),
        authors.len(),
        pub_dates.len()
      );
      return Err(Error::Message(msg));
    }

    let iter = itertools::multizip((titles, links, bodies, authors, pub_dates));

    for (title, link, body, author, pub_date) in iter {
      let mut post = self.prepare_template(&post);
      self.apply_template(
        &mut post,
        &title,
        &link,
        body.as_deref(),
        author.as_deref(),
        pub_date,
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

fn parse_date_from_element(
  elem: ElementRef<'_>,
) -> Option<DateTime<FixedOffset>> {
  fn parse_standard_date(s: &str) -> Option<DateTime<FixedOffset>> {
    // ISO 8601 date (1996-12-19T16:39:57-08:00)
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
      return Some(d);
    }

    // RFC 2822 date (Tue, 19 Dec 1996 16:39:57 -0800)
    if let Ok(d) = DateTime::parse_from_rfc2822(s) {
      return Some(d);
    }

    None
  }

  let text = elem.text().collect::<String>();
  if let Some(d) = parse_standard_date(&text) {
    return Some(d);
  }

  for (_name, attr) in elem.value().attrs() {
    if let Some(d) = parse_standard_date(attr) {
      return Some(d);
    }
  }

  None
}

#[async_trait::async_trait]
impl FeedFilter for Split {
  async fn run(
    &self,
    _ctx: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    let mut posts = vec![];
    for post in feed.take_posts() {
      let mut split_posts = self.split(post)?;
      posts.append(&mut split_posts);
    }

    feed.set_posts(posts);
    Ok(feed)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test_utils::assert_filter_parse;

  #[test]
  fn test_parse_config() {
    assert_filter_parse(
      r#"
remove_element:
 - 'img[src$=".gif"]'
 - 'span.ads'
"#,
      RemoveElementConfig {
        selectors: vec!["img[src$=\".gif\"]".into(), "span.ads".into()],
      },
    );

    assert_filter_parse(
      r#"
keep_element: img[src$=".gif"]
    "#,
      KeepElementConfig {
        selector: "img[src$=\".gif\"]".into(),
      },
    );
  }
}
