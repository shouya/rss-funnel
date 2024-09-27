use chrono::{DateTime, FixedOffset};
use serde::Serialize;

#[derive(Debug, Serialize, Hash, PartialEq, Eq, Default)]
pub struct NormalizedFeed {
  pub title: String,
  pub link: String,
  pub description: Option<String>,
  pub posts: Vec<NormalizedPost>,
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash, Default)]
pub struct NormalizedPost {
  pub title: String,
  pub author: Option<String>,
  pub link: String,
  pub body: Option<String>,
  pub date: Option<DateTime<FixedOffset>>,
}

impl NormalizedPost {
  pub fn into_rss_item(self) -> rss::Item {
    let guid = rss::Guid {
      value: self.link.clone(),
      permalink: true,
    };

    rss::Item {
      title: Some(self.title),
      link: Some(self.link),
      description: self.body,
      pub_date: self.date.map(|d| d.to_rfc3339()),
      author: self.author,
      guid: Some(guid),
      ..Default::default()
    }
  }

  pub fn into_atom_entry(self) -> atom_syndication::Entry {
    atom_syndication::Entry {
      title: atom_syndication::Text::plain(self.title),
      id: self.link.clone(),
      links: vec![atom_syndication::Link {
        href: self.link,
        ..Default::default()
      }],
      authors: self
        .author
        .into_iter()
        .map(|a| atom_syndication::Person {
          name: a,
          ..Default::default()
        })
        .collect(),
      updated: self
        .date
        .unwrap_or_else(|| chrono::Utc::now().fixed_offset()),
      published: self.date,
      content: self.body.map(|b| atom_syndication::Content {
        value: Some(b),
        ..Default::default()
      }),
      ..Default::default()
    }
  }
}
