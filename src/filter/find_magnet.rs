use regex::Regex;
use rss::Enclosure;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
  feed::{Feed, Post},
  util::{ConfigError, Error},
};

use super::{FeedFilter, FeedFilterConfig, FilterContext};

#[derive(
  JsonSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash,
)]
/// Find magnet link discovered in the body of entries and save it in
/// the enclosure (RSS)/link (Atom). The resulting feed can be used in
/// a torrent client.
pub struct FindMagnetConfig {
  /// Match any `[a-fA-F0-9]{40}` as the info hash.
  #[serde(default)]
  info_hash: bool,
  /// Whether or not to override existing magnet links in the enclosure/link.
  #[serde(default)]
  override_existing: bool,
}

pub struct FindMagnet {
  config: FindMagnetConfig,
}

#[async_trait::async_trait]
impl FeedFilterConfig for FindMagnetConfig {
  type Filter = FindMagnet;

  async fn build(self) -> Result<Self::Filter, ConfigError> {
    Ok(FindMagnet { config: self })
  }
}

#[async_trait::async_trait]
impl FeedFilter for FindMagnet {
  async fn run(
    &self,
    _ctx: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed, Error> {
    let mut posts = feed.take_posts();

    for post in posts.iter_mut() {
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
}

lazy_static::lazy_static! {
  static ref MAGNET_LINK_REGEX: Regex = Regex::new(
    r"(?i)\b(?P<full>magnet:\?xt=urn:btih:[a-fA-F0-9]{40}(&\w+=[^\s]+)*)\b"
  )
    .unwrap();
  static ref INFO_HASH_REGEX: Regex =
    Regex::new(r"\b(?i)(?P<info_hash>[a-fA-F0-9]{40})\b").unwrap();
}

fn existing_magnet_link(post: &Post) -> Option<&str> {
  match post {
    Post::Rss(p) => p
      .enclosure()
      .into_iter()
      .filter(|e| e.mime_type() == "application/x-bittorrent")
      .map(|e| e.url())
      .next(),
    Post::Atom(p) => p
      .links()
      .iter()
      .filter(|l| l.href().starts_with("magnet:"))
      .map(|l| l.href())
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
        length: "".to_string(),
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

fn find_magnet_links(text: &str, config: &FindMagnetConfig) -> Vec<String> {
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
        format!(
          "magnet:?xt=urn:btih:{}",
          m.name("info_hash").unwrap().as_str()
        )
      } else {
        m.name("full").unwrap().as_str().to_string()
      }
    })
    .collect()
}
