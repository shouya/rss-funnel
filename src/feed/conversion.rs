#![allow(clippy::field_reassign_with_default)]
// Utility wrapper type to get around orphan rules for implementing
// traits on foreign types.
pub(super) struct W<T>(pub T);

// The Extension struct and ExtensionMap type alias in the two
// libraries are defined identically. Therefore, we can convert
// between them by copying the fields.

impl From<W<rss::extension::Extension>>
  for atom_syndication::extension::Extension
{
  fn from(W(ext): W<rss::extension::Extension>) -> Self {
    let rss::extension::Extension {
      name,
      value,
      attrs,
      children,
    } = ext;

    let children = children
      .into_iter()
      .map(|(k, v)| (k, v.into_iter().map(|e| W(e).into()).collect()))
      .collect();

    atom_syndication::extension::Extension {
      name,
      value,
      attrs,
      children,
    }
  }
}

impl From<W<atom_syndication::extension::Extension>>
  for rss::extension::Extension
{
  fn from(W(ext): W<atom_syndication::extension::Extension>) -> Self {
    let atom_syndication::extension::Extension {
      name,
      value,
      attrs,
      children,
    } = ext;

    let children = children
      .into_iter()
      .map(|(k, v)| (k, v.into_iter().map(|e| W(e).into()).collect()))
      .collect();

    rss::extension::Extension {
      name,
      value,
      attrs,
      children,
    }
  }
}

impl From<W<atom_syndication::extension::ExtensionMap>>
  for rss::extension::ExtensionMap
{
  fn from(W(ext): W<atom_syndication::extension::ExtensionMap>) -> Self {
    ext
      .into_iter()
      .map(|(k, v)| {
        let v = v
          .into_iter()
          .map(|(k, v)| (k, v.into_iter().map(|e| W(e).into()).collect()))
          .collect();
        (k, v)
      })
      .collect()
  }
}

impl From<W<rss::extension::ExtensionMap>>
  for atom_syndication::extension::ExtensionMap
{
  fn from(W(ext): W<rss::extension::ExtensionMap>) -> Self {
    ext
      .into_iter()
      .map(|(k, v)| {
        let v = v
          .into_iter()
          .map(|(k, v)| (k, v.into_iter().map(|e| W(e).into()).collect()))
          .collect();
        (k, v)
      })
      .collect()
  }
}

impl From<W<rss::Channel>> for atom_syndication::Feed {
  fn from(W(channel): W<rss::Channel>) -> Self {
    use atom_syndication::{Category, FixedDateTime, Generator, Link, Person};
    let parse_date = |s: &str| FixedDateTime::parse_from_rfc3339(s).ok();

    let mut feed = atom_syndication::Feed::default();

    // Title and ID already set
    feed.title = channel.title.into();
    feed.id = channel.link.clone();

    // Updated - using last_build_date if available, otherwise pub_date
    feed.updated = channel
      .last_build_date
      .as_deref()
      .or(channel.pub_date.as_deref())
      .and_then(parse_date)
      .unwrap_or_default();

    // Authors - Assuming managing_editor as the author if available
    if let Some(editor) = channel.managing_editor {
      let mut person = Person::default();
      person.name = editor;
      feed.authors.push(person);
    }

    // Links - Primary link to the channel's website
    let mut link = Link::default();
    link.href = channel.link;
    feed.links.push(link);

    // Categories
    for category in channel.categories {
      let mut cat = Category::default();
      cat.term = category.name;
      feed.categories.push(cat);
    }

    // Generator - Assuming it's a simple string without version or uri
    if let Some(generator_str) = channel.generator {
      let generator = Generator {
        value: generator_str,
        version: None,
        uri: None,
      };
      feed.generator = Some(generator);
    }

    // Language as lang
    feed.lang = channel.language;

    // Subtitle as a description
    if !channel.description.is_empty() {
      feed.subtitle = Some(channel.description.into());
    }

    feed.extensions = W(channel.extensions).into();

    // Entries
    feed.entries = channel
      .items
      .into_iter()
      .map(W)
      .map(atom_syndication::Entry::from)
      .collect();

    feed
  }
}

impl From<W<rss::Item>> for atom_syndication::Entry {
  fn from(W(item): W<rss::Item>) -> Self {
    use atom_syndication::{Content, Entry, FixedDateTime, Link, Person, Text};

    let parse_date = |s: &str| FixedDateTime::parse_from_rfc3339(s).ok();

    let mut entry = Entry::default();

    entry.title = item.title.map_or_else(Text::default, |t| t.into());
    entry.id = item.guid.map_or_else(String::default, |g| g.value);

    if let Some(pub_date) = item.pub_date.as_deref().and_then(parse_date) {
      entry.updated = pub_date;
      entry.published = Some(pub_date);
    } else {
      entry.updated = FixedDateTime::default();
    }

    if let Some(author_email) = item.author {
      let mut person = Person::default();
      person.name = author_email;
      entry.authors.push(person);
    }

    item.categories.into_iter().for_each(|cat| {
      let mut category = atom_syndication::Category::default();
      category.term = cat.name;
      entry.categories.push(category);
    });

    if let Some(link) = item.link.as_ref() {
      let mut atom_link = Link::default();
      atom_link.href = link.clone();
      entry.links.push(atom_link);
    }

    entry.summary = item.description.map(|d| d.into());

    if let Some(content) = item.content {
      let mut atom_content = Content::default();
      atom_content.value = Some(content);
      entry.content = Some(atom_content);
    }

    entry.extensions = W(item.extensions).into();

    entry
  }
}

impl From<W<atom_syndication::Entry>> for rss::Item {
  fn from(W(entry): W<atom_syndication::Entry>) -> Self {
    let mut item = rss::Item::default();

    item.title = Some(entry.title.as_str().to_owned());
    item.link = entry.links.first().map(|l| l.href.clone());
    item.description = entry.summary.map(|s| s.as_str().to_owned());
    item.author = entry.authors.first().map(|a| a.name.clone());
    item.pub_date = entry.published.map(|d| d.to_rfc2822());
    item.guid = Some(rss::Guid {
      value: entry.id,
      permalink: false,
    });
    item.content = entry.content.and_then(|c| c.value);

    item.extensions = W(entry.extensions).into();

    item.categories = entry
      .categories
      .into_iter()
      .map(|c| rss::Category {
        name: c.term,
        domain: None,
      })
      .collect();

    item
  }
}

impl From<W<atom_syndication::Feed>> for rss::Channel {
  fn from(W(feed): W<atom_syndication::Feed>) -> Self {
    let mut channel = rss::Channel::default();

    channel.title = feed.title.as_str().to_owned();
    channel.link = feed
      .links
      .first()
      .map_or(String::default(), |l| l.href.clone());
    channel.description = feed
      .subtitle
      .map_or(String::default(), |s| s.as_str().to_owned());
    channel.last_build_date = Some(feed.updated.to_rfc2822());
    channel.language = feed.lang;
    channel.generator = feed.generator.map(|g| g.value);

    channel.items = feed
      .entries
      .into_iter()
      .map(W)
      .map(rss::Item::from)
      .collect();

    channel.extensions = W(feed.extensions).into();

    channel.categories = feed
      .categories
      .into_iter()
      .map(|c| rss::Category {
        name: c.term,
        domain: None,
      })
      .collect();

    channel
  }
}
