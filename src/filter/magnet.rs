use regex::Regex;
use rss::Enclosure;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{
  ConfigError, Error,
  feed::{Feed, Post},
};

use super::{FeedFilter, FeedFilterConfig, FilterContext};

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
/// Find magnet link discovered in the body of entries and save it in
/// the enclosure (RSS)/link (Atom). The resulting feed can be used in
/// a torrent client.
pub struct MagnetConfig {
  /// Match any `[a-fA-F0-9]{40}` as the info hash.
  #[serde(default)]
  info_hash: bool,
  /// Whether or not to override existing magnet links in the enclosure/link.
  #[serde(default)]
  override_existing: bool,
}

pub struct Magnet {
  config: MagnetConfig,
}

#[async_trait::async_trait]
impl FeedFilterConfig for MagnetConfig {
  type Filter = Magnet;

  async fn build(self) -> Result<Self::Filter, ConfigError> {
    Ok(Magnet { config: self })
  }
}

#[async_trait::async_trait]
impl FeedFilter for Magnet {
  async fn run(
    &self,
    _ctx: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed, Error> {
    let mut posts = feed.take_posts();

    for post in &mut posts {
      let bodies = post.bodies();
      let link: Option<String> = bodies
        .iter()
        .flat_map(|body| find_magnet_links(body, &self.config))
        .next();

      if let Some(link) = link {
        set_magnet_link(post, link, self.config.override_existing);
      }
    }

    feed.set_posts(posts);
    Ok(feed)
  }

  fn cache_granularity(&self) -> super::CacheGranularity {
    super::CacheGranularity::FeedAndPost
  }
}

lazy_static::lazy_static! {
  static ref MAGNET_LINK_REGEX: Regex = Regex::new(
    // btih: bt info hash v1; btmh: bt info hash v2
    r"\b(?<full>magnet:\?xt=urn:bt(ih:[a-fA-F0-9]{40}|mh:[a-fA-F0-9]{68})(&[\w.]+=[^\s]+)*)\b"
  )
    .unwrap();
  static ref INFO_HASH_REGEX: Regex =
    Regex::new(r"\b(?<info_hash>[a-fA-F0-9]{40}|[a-fA-F0-9]{68})\b").unwrap();
}

fn existing_magnet_link(post: &Post) -> Option<&str> {
  match post {
    Post::Rss(p) => p
      .enclosure()
      .into_iter()
      .filter(|e| e.mime_type() == "application/x-bittorrent")
      .map(rss::Enclosure::url)
      .next(),
    Post::Atom(p) => p
      .links()
      .iter()
      .filter(|l| l.href().starts_with("magnet:"))
      .map(atom_syndication::Link::href)
      .next(),
  }
}

fn set_magnet_link(post: &mut Post, link: String, override_: bool) {
  if !override_ && existing_magnet_link(post).is_none() {
    return;
  }

  match post {
    Post::Rss(p) => {
      let enclosure = Enclosure {
        url: link,
        mime_type: "application/x-bittorrent".to_string(),
        length: String::new(),
      };
      p.set_enclosure(enclosure);
    }
    Post::Atom(p) => {
      let link = atom_syndication::Link {
        href: link,
        mime_type: Some("application/x-bittorrent".to_string()),
        ..Default::default()
      };
      p.links.push(link);
    }
  }
}

fn find_magnet_links(text: &str, config: &MagnetConfig) -> Vec<String> {
  let regex = if config.info_hash {
    &*INFO_HASH_REGEX
  } else {
    &*MAGNET_LINK_REGEX
  };

  let captures: Vec<regex::Captures> = regex.captures_iter(text).collect();

  captures
    .into_iter()
    .map(|m| {
      if config.info_hash {
        let info_hash = m.name("info_hash").unwrap().as_str();
        if info_hash.len() == 40 {
          format!("magnet:?xt=urn:btih:{info_hash}")
        } else if info_hash.len() == 68 {
          format!("magnet:?xt=urn:btmh:{info_hash}")
        } else {
          warn!("Bad length for info hash: {info_hash}");
          format!("magnet:?xt=urn:btih:{info_hash}")
        }
      } else {
        m.name("full").unwrap().as_str().to_string()
      }
    })
    .collect()
}

#[cfg(test)]
mod test {
  #[test]
  fn test_find_magnet_links() {
    let text = "HELLO magnet:?xt=urn:btih:1234567890ABCDEF1234567890ABCDEF12345678&dn=hello+world WORLD";
    let links = super::find_magnet_links(
      text,
      &super::MagnetConfig {
        info_hash: false,
        override_existing: false,
      },
    );
    assert_eq!(
      links,
      vec![
        "magnet:?xt=urn:btih:1234567890ABCDEF1234567890ABCDEF12345678&dn=hello+world"
      ]
    );

    let text = "HELLO 1234567890ABCDEF1234567890ABCDEF12345678 WORLD";
    let links = super::find_magnet_links(
      text,
      &super::MagnetConfig {
        info_hash: true,
        override_existing: false,
      },
    );
    assert_eq!(
      links,
      vec!["magnet:?xt=urn:btih:1234567890ABCDEF1234567890ABCDEF12345678"]
    );

    let text = "HELLO 1234567890ABCDEF1234567890ABCDEF12345678 WORLD";
    let links = super::find_magnet_links(
      text,
      &super::MagnetConfig {
        info_hash: false,
        override_existing: false,
      },
    );
    assert!(links.is_empty());
  }
}
