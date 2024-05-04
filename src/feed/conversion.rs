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

    Self {
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
      .map(|(k, v)| (k, v.into_iter().map(W).map(Into::into).collect()))
      .collect();

    Self {
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
          .map(|(k, v)| (k, v.into_iter().map(W).map(Into::into).collect()))
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
          .map(|(k, v)| (k, v.into_iter().map(W).map(Into::into).collect()))
          .collect();
        (k, v)
      })
      .collect()
  }
}

impl From<W<rss::Channel>> for atom_syndication::Feed {
  fn from(W(channel): W<rss::Channel>) -> Self {
    use atom_syndication::{
      Category, FixedDateTime, Generator, Link, Person, Text,
    };

    let parse_date = |s: &str| FixedDateTime::parse_from_rfc2822(s).ok();

    let mut feed = Self::default();

    feed.title = channel.title.into();
    feed.id.clone_from(&channel.link);

    // Updated - using last_build_date if available, otherwise pub_date
    feed.updated = channel
      .last_build_date
      .or(channel.pub_date)
      .as_deref()
      .and_then(parse_date)
      .unwrap_or_default();

    // Authors - Assuming managing_editor as the author if available
    if let Some(editor) = channel.managing_editor {
      let person = Person {
        name: editor,
        ..Default::default()
      };
      feed.authors.push(person);
    }

    // Links - Primary link to the channel's website
    let link = Link {
      href: channel.link,
      ..Default::default()
    };
    feed.links.push(link);

    // Categories
    feed.categories = channel
      .categories
      .into_iter()
      .map(|category| Category {
        term: category.name,
        ..Default::default()
      })
      .collect();

    // Generator - Assuming it's a simple string without version or uri
    feed.generator = channel.generator.map(|value| Generator {
      value,
      ..Default::default()
    });

    // Language as lang
    feed.lang = channel.language;

    // Subtitle as a description
    feed.subtitle = Some(channel.description.into());

    // Rights
    feed.rights = channel.copyright.as_deref().map(Text::from);

    // Image and logo
    if let Some(image) = channel.image {
      feed.icon = Some(image.url.clone());
      feed.logo = Some(image.url);
    }

    // Extensions
    feed.extensions = W(channel.extensions).into();

    // Entries
    feed.entries = channel.items.into_iter().map(W).map(Into::into).collect();

    feed
  }
}

impl From<W<atom_syndication::Feed>> for rss::Channel {
  fn from(W(feed): W<atom_syndication::Feed>) -> Self {
    let mut channel = Self::default();

    feed.title.as_str().clone_into(&mut channel.title);
    channel.link = feed
      .links
      .into_iter()
      .next()
      .map_or_else(String::default, |l| l.href);
    feed
      .subtitle
      .as_deref()
      .unwrap_or_default()
      .clone_into(&mut channel.description);

    if feed.updated.timestamp() != 0 {
      channel.last_build_date = Some(feed.updated.to_rfc2822());
    }
    channel.language = feed.lang;
    channel.generator = feed.generator.map(|g| g.value);
    channel.items = feed.entries.into_iter().map(W).map(Into::into).collect();
    channel.extensions = W(feed.extensions).into();
    channel.managing_editor = feed.authors.into_iter().next().map(|a| a.name);

    channel.categories = feed
      .categories
      .into_iter()
      .map(|c| rss::Category {
        name: c.term,
        ..Default::default()
      })
      .collect();

    channel
  }
}

impl From<W<rss::Item>> for atom_syndication::Entry {
  fn from(W(item): W<rss::Item>) -> Self {
    use atom_syndication::{Content, Entry, FixedDateTime, Link, Person, Text};

    let parse_date = |s: &str| FixedDateTime::parse_from_rfc2822(s).ok();

    let mut entry = Entry::default();

    entry.title = item.title.map_or_else(Text::default, |t| t.into());
    entry.id = item
      .guid
      .map(|g| g.value)
      .or_else(|| item.link.clone())
      .unwrap_or_default();

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

    if let Some(link) = item.link {
      let mut atom_link = Link::default();
      atom_link.href = link;
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

    item.title = Some(entry.title.value);
    item.link = entry.links.into_iter().next().map(|l| l.href);
    item.description = entry.summary.map(|s| s.value);
    item.author = entry.authors.into_iter().next().map(|a| a.name);
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
        ..Default::default()
      })
      .collect();

    item
  }
}
