use super::{FeedFilter, FeedFilterConfig};
use ego_tree::{NodeId, NodeMut};
use regex::{Regex, RegexBuilder, RegexSet, RegexSetBuilder};
use scraper::{Html, Node};
use serde::{Deserialize, Serialize};

use crate::{
  html::fragment_root_node_id,
  util::{ConfigError, Result},
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HighlightConfig {
  #[serde(flatten)]
  keywords: KeywordsOrPatterns,
  bg_color: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
enum KeywordsOrPatterns {
  Keywords {
    keywords: Vec<String>,
  },
  Patterns {
    patterns: serde_regex::Serde<Vec<Regex>>,
  },
}

impl KeywordsOrPatterns {
  fn into_patterns(self) -> Result<Vec<String>> {
    match self {
      Self::Keywords { keywords } => {
        let patterns = keywords
          .into_iter()
          .map(|k| regex::escape(&k))
          .collect::<Vec<_>>();
        Ok(patterns)
      }
      Self::Patterns { patterns } => {
        let patterns = patterns
          .into_inner()
          .into_iter()
          .map(|r| r.as_str().to_owned())
          .collect();
        Ok(patterns)
      }
    }
  }
}

#[async_trait::async_trait]
impl FeedFilterConfig for HighlightConfig {
  type Filter = Highlight;

  async fn build(self) -> Result<Self::Filter> {
    let patterns = self.keywords.into_patterns()?;
    let bg_color = self.bg_color.unwrap_or_else(|| "#ffff00".into());
    Highlight::new(&patterns, bg_color)
  }
}

pub struct Highlight {
  bg_color: String,
  regexset: RegexSet,
  patterns: Vec<Regex>,
}

enum TextSegment {
  Text(String),
  Highlight(String),
}

impl TextSegment {
  /// Returns the node id of the newly inserted node
  fn insert(
    self,
    color: &str,
    node: &mut NodeMut<'_, scraper::Node>,
  ) -> NodeId {
    use scraper::node::Text;

    match self {
      Self::Text(text) => {
        let new_node = Node::Text(Text { text: text.into() });
        node.insert_after(new_node).id()
      }
      Self::Highlight(text) => {
        // HACK: scraper doesn't provide a way to constructor an Element. So
        // we have to parse it from a string.
        let fragment = format!(
          "<span style=\"background-color: {}\" class=\"rss-funnel-hl\">{}</span>",
          // TODO: escape the two fields?
          color,
          text
        );

        insert_sibling_fragment(node, &fragment)
      }
    }
  }
}

impl Highlight {
  fn new<T: AsRef<str>>(patterns: &[T], bg_color: String) -> Result<Self> {
    let regexset = RegexSetBuilder::new(patterns)
      .case_insensitive(true)
      .build()
      .map_err(ConfigError::from)?;
    let patterns = patterns
      .iter()
      .map(|p| {
        RegexBuilder::new(p.as_ref())
          .case_insensitive(true)
          .build()
          .map_err(ConfigError::from)
          .map_err(|e| e.into())
      })
      .collect::<Result<Vec<Regex>>>()?;

    Ok(Self {
      patterns,
      regexset,
      bg_color,
    })
  }

  fn highlight_html(&self, description: &str) -> String {
    let mut html = Html::parse_fragment(description);
    let text_node_ids: Vec<NodeId> = html
      .tree
      .nodes()
      .filter_map(|node| match node.value() {
        Node::Text(_) => Some(node.id()),
        _ => None,
      })
      .collect();

    for node_id in text_node_ids {
      let mut node = html.tree.get_mut(node_id).expect("unreachable");
      self.highlight_text_node(&mut node);
    }

    html.html()
  }

  fn highlight_text_node(&self, node: &mut NodeMut<'_, Node>) {
    let text = match node.value() {
      Node::Text(text) => text.to_string(),
      _ => return,
    };

    if !self.regexset.is_match(&text) {
      return;
    }

    let segments = self.segmentize_text(&text);
    match node.value() {
      Node::Text(text) => text.text.clear(),
      _ => return,
    };

    let mut next_node_id = node.id();
    for segment in segments {
      let mut node = node.tree().get_mut(next_node_id).unwrap();
      next_node_id = segment.insert(&self.bg_color, &mut node);
    }
  }

  fn segmentize_text(&self, text: &str) -> Vec<TextSegment> {
    let mut cursor = 0;
    let mut out = vec![];
    while cursor < text.len() {
      let set_matches = self.regexset.matches_at(text, cursor);
      if !set_matches.matched_any() {
        break;
      }

      // find the first matching regex
      let m = set_matches
        .iter()
        .map(|i| {
          let m = self.patterns[i]
            .find_at(text, cursor)
            .expect("regex match failed");
          (m.start(), m)
        })
        .min_by_key(|(start, _)| *start)
        .map(|(_, m)| m)
        .into_iter()
        .next()
        .expect("regex match failed");

      if m.start() > cursor {
        out.push(TextSegment::Text(text[cursor..m.start()].into()));
      }

      out.push(TextSegment::Highlight(text[m.start()..m.end()].into()));
      cursor = m.end();
    }

    if cursor < text.len() {
      out.push(TextSegment::Text(text[cursor..].into()));
    }

    out
  }
}

#[async_trait::async_trait]
impl FeedFilter for Highlight {
  async fn run(&self, feed: &mut crate::feed::Feed) -> Result<()> {
    let mut posts = feed.take_posts();

    for post in &mut posts {
      if let Some(description) = post.description_mut() {
        *description = self.highlight_html(description);
      }
    }

    feed.set_posts(posts);

    Ok(())
  }
}

fn insert_sibling_fragment(
  node: &mut NodeMut<'_, Node>,
  fragment: &str,
) -> NodeId {
  let new_tree = scraper::Html::parse_fragment(fragment).tree;
  let new_root = node.tree().extend_tree(new_tree);
  let root_node_id = fragment_root_node_id(new_root.into());
  node.insert_id_after(root_node_id).id()
}

#[cfg(test)]
mod test {
  use super::*;
  #[test]
  fn test_highlighting() {
    let keywords = vec!["foo", "bar"];
    let highlight = Highlight::new(&keywords, "#ffff00".into())
      .expect("failed to build highlighter");

    let html = r#"<html><p class="foo">FOO<div><!-- bar -->foo<br> bar</div></p></html>
"#;
    let actual = highlight.highlight_html(html);
    let expected = r#"<html><p class="foo"><span class="rss-funnel-hl" style="background-color: #ffff00">FOO</span><div><!-- bar --><span class="rss-funnel-hl" style="background-color: #ffff00">foo</span><br> <span style="background-color: #ffff00" class="rss-funnel-hl">bar</span></div></p></html>
"#;

    // println!("{}", actual);
    // println!("{}", expected);

    assert_eq!(
      Html::parse_fragment(&actual).tree,
      Html::parse_fragment(expected).tree
    );
  }
}
