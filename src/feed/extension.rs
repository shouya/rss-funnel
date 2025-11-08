use std::collections::BTreeMap;

pub struct TagRef<'a> {
  pub name: &'a String,
  #[allow(dead_code)]
  pub attrs: &'a BTreeMap<String, String>,
  pub value: &'a Option<String>,
}

pub struct TagRefMut<'a> {
  pub name: &'a mut String,
  #[allow(dead_code)]
  pub attrs: &'a mut BTreeMap<String, String>,
  pub value: &'a mut Option<String>,
}

pub trait ExtensionExt {
  fn tags(&self) -> Vec<TagRef<'_>>;
  fn tags_mut(&mut self) -> Vec<TagRefMut<'_>>;

  fn tags_mut_with_names(&mut self, names: &[&str]) -> Vec<TagRefMut<'_>> {
    self
      .tags_mut()
      .into_iter()
      .filter(|tag| names.contains(&tag.name.as_str()))
      .collect()
  }

  fn tags_with_names(&self, names: &[&str]) -> Vec<TagRef<'_>> {
    self
      .tags()
      .into_iter()
      .filter(|tag| names.contains(&tag.name.as_str()))
      .collect()
  }
}

macro_rules! impl_extension_ext {
  ($ty:ty) => {
    impl ExtensionExt for $ty {
      fn tags(&self) -> Vec<TagRef<'_>> {
        let tag = TagRef {
          name: &self.name,
          attrs: &self.attrs,
          value: &self.value,
        };

        let mut tags = vec![tag];
        for children in self.children.values() {
          tags.extend(children.iter().flat_map(|ext| ext.tags()));
        }
        tags
      }

      fn tags_mut(&mut self) -> Vec<TagRefMut<'_>> {
        let tag = TagRefMut {
          name: &mut self.name,
          attrs: &mut self.attrs,
          value: &mut self.value,
        };

        let mut tags = vec![tag];
        for children in self.children.values_mut() {
          tags.extend(children.iter_mut().flat_map(|ext| ext.tags_mut()));
        }
        tags
      }
    }
  };
}

// These two structs has exactly the same structure but are different
// types since they belong to different crates.
impl_extension_ext!(atom_syndication::extension::Extension);
impl_extension_ext!(rss::extension::Extension);

impl<T> ExtensionExt for BTreeMap<String, BTreeMap<String, Vec<T>>>
where
  T: ExtensionExt,
{
  fn tags(&self) -> Vec<TagRef<'_>> {
    self
      .values()
      .flat_map(|children| {
        children
          .values()
          .flat_map(|exts| exts.iter().flat_map(ExtensionExt::tags))
      })
      .collect()
  }

  fn tags_mut(&mut self) -> Vec<TagRefMut<'_>> {
    self
      .values_mut()
      .flat_map(|children| {
        children
          .values_mut()
          .flat_map(|exts| exts.iter_mut().flat_map(ExtensionExt::tags_mut))
      })
      .collect()
  }
}
