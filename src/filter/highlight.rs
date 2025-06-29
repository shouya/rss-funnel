use super::{FeedFilter, FeedFilterConfig, FilterContext};
use ego_tree::{NodeId, NodeMut};
use regex::{Regex, RegexBuilder, RegexSet, RegexSetBuilder};
use schemars::JsonSchema;
use scraper::{Html, Node};
use serde::{Deserialize, Serialize};

use crate::{feed::Feed, util::fragment_root_node_id, ConfigError, Result};

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
/// Highlight the given keywords in the post's body
pub struct HighlightConfig {
  #[serde(flatten)]
  keywords: KeywordsOrPatterns,
  /// Background color to use for highlighting. Default is yellow.
  #[serde(default)]
  bg_color: Option<String>,
  #[serde(default)]
  case_sensitive: Option<bool>,
}

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
#[serde(untagged)]
enum KeywordsOrPatterns {
  /// A list of keywords to highlight
  Keywords { keywords: Vec<String> },
  /// A list of regex patterns to highlight
  Patterns { patterns: Vec<String> },
}

impl KeywordsOrPatterns {
  fn into_patterns(self) -> Result<Vec<String>, ConfigError> {
    match self {
      Self::Keywords { keywords } => {
        let patterns = keywords
          .into_iter()
          .map(|k| regex::escape(&k))
          .collect::<Vec<_>>();
        Ok(patterns)
      }
      Self::Patterns { patterns } => Ok(patterns),
    }
  }
}

#[async_trait::async_trait]
impl FeedFilterConfig for HighlightConfig {
  type Filter = Highlight;

  async fn build(self) -> Result<Self::Filter, ConfigError> {
    let patterns = self.keywords.into_patterns()?;
    let bg_color = self.bg_color.unwrap_or_else(|| "#ffff00".into());
    let case_sensitive = self.case_sensitive.unwrap_or(false);
    Highlight::new(&patterns, bg_color, case_sensitive)
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
      #[allow(clippy::uninlined_format_args)]
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
  fn new<T: AsRef<str>>(
    patterns: &[T],
    bg_color: String,
    case_sensitive: bool,
  ) -> Result<Self, ConfigError> {
    let regexset = RegexSetBuilder::new(patterns)
      .case_insensitive(!case_sensitive)
      .build()?;
    let patterns = patterns
      .iter()
      .map(|p| {
        RegexBuilder::new(p.as_ref())
          .case_insensitive(!case_sensitive)
          .build()
      })
      .collect::<Result<Vec<Regex>, _>>()?;

    Ok(Self {
      bg_color,
      regexset,
      patterns,
    })
  }

  fn highlight_html(&self, body: &str) -> String {
    let mut html = Html::parse_fragment(body);
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
    }

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
  async fn run(
    &self,
    _ctx: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    let mut posts = feed.take_posts();

    for post in &mut posts {
      post.modify_bodies(|body| {
        *body = self.highlight_html(body);
      });
    }

    feed.set_posts(posts);

    Ok(feed)
  }

  fn cache_granularity(&self) -> super::CacheGranularity {
    super::CacheGranularity::FeedAndPost
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
  use crate::test_utils::assert_filter_parse;

  use super::*;
  #[test]
  fn test_highlighting() {
    let keywords = vec!["foo", "bar"];
    let highlight = Highlight::new(&keywords, "#ffff00".into(), false)
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

  #[test]
  fn test_parse_config() {
    assert_filter_parse(
      r"
highlight:
  keywords:
    - foo
    - bar
  bg_color: '#ffff00'
    ",
      HighlightConfig {
        keywords: KeywordsOrPatterns::Keywords {
          keywords: vec!["foo".into(), "bar".into()],
        },
        bg_color: Some("#ffff00".into()),
        case_sensitive: None,
      },
    );

    assert_filter_parse(
      r"
highlight:
  patterns:
    - '\bfoo\b'
  bg_color: '#ffff00'
",
      HighlightConfig {
        keywords: KeywordsOrPatterns::Patterns {
          patterns: vec![r"\bfoo\b".into()],
        },
        bg_color: Some("#ffff00".into()),
        case_sensitive: None,
      },
    );
  }
}
