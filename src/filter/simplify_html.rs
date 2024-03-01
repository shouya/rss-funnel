use readability::extractor::extract;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::feed::Feed;
use crate::util::Result;

use super::{FeedFilter, FeedFilterConfig, FilterContext};

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone)]
/// The simplify_html filter simplifies the HTML content of
/// posts. There is no configuration.
pub struct SimplifyHtmlConfig {}

pub struct SimplifyHtmlFilter;

#[async_trait::async_trait]
impl FeedFilterConfig for SimplifyHtmlConfig {
  type Filter = SimplifyHtmlFilter;

  async fn build(self) -> Result<Self::Filter> {
    Ok(SimplifyHtmlFilter)
  }
}

#[async_trait::async_trait]
impl FeedFilter for SimplifyHtmlFilter {
  async fn run(
    &self,
    _ctx: &mut FilterContext,
    mut feed: Feed,
  ) -> Result<Feed> {
    let mut posts = feed.take_posts();

    for post in &mut posts {
      let link = post.link().unwrap_or("").to_string();
      if let Some(description) = post.description_mut() {
        if let Some(simplified) = simplify(description, &link) {
          *description = simplified;
        }
      };
    }

    feed.set_posts(posts);
    Ok(feed)
  }
}

pub(super) fn simplify(text: &str, url: &str) -> Option<String> {
  let url = Url::parse(url).ok()?;
  let mut text = std::io::Cursor::new(text);
  let product = extract(&mut text, &url).ok()?;
  Some(product.content)
}
