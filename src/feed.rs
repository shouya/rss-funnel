use axum::response::IntoResponse;
use http::StatusCode;
use paste::paste;
use serde::Deserialize;
use serde::Serialize;

use crate::util::Error;
use crate::util::Result;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Feed {
  Rss(rss::Channel),
}

impl Feed {
  pub fn from_rss_content(content: &str) -> Result<Self> {
    let cursor = std::io::Cursor::new(content);
    let channel = rss::Channel::read_from(cursor)?;
    Ok(Feed::Rss(channel))
  }

  pub fn into_resp(self) -> Result<impl IntoResponse> {
    let headers = [(http::header::CONTENT_TYPE, "application/rss+xml")];
    match self {
      Feed::Rss(channel) => {
        let body = channel.to_string();
        Ok((StatusCode::OK, headers, body))
      }
    }
  }

  pub fn take_posts(&mut self) -> Vec<Post> {
    match self {
      Feed::Rss(channel) => {
        let posts = channel.items.split_off(0);
        posts.into_iter().map(Post::Rss).collect()
      }
    }
  }

  pub fn set_posts(&mut self, posts: Vec<Post>) {
    match self {
      Feed::Rss(channel) => {
        channel.items = posts
          .into_iter()
          .filter_map(|post| match post {
            Post::Rss(item) => Some(item),
          })
          .collect();
      }
    }
  }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Post {
  Rss(rss::Item),
}

macro_rules! impl_post_get {
  ($($key:ident),*) => {
    paste! {
      impl Post {
        $(
        #[allow(unused)]
        pub fn $key(&self) -> Option<&str> {
          match self {
            Post::Rss(item) => item.$key.as_deref(),
          }
        }

        #[allow(unused)]
        pub fn [<$key _mut>](&mut self) -> Option<&mut String> {
          match self {
            Post::Rss(item) => item.$key.as_mut(),
          }
        }

        #[allow(unused)]
        pub fn [<$key _or_err>](&self) -> Result<&str> {
          match self.$key() {
            Some(value) => Ok(value),
            None => Err(Error::FeedParse(concat!("missing ", stringify!($key)))),
          }
        }

        #[allow(unused)]
        pub fn [<$key _or_insert>](&mut self) -> &mut String {
          match self {
            Post::Rss(item) => item.$key.get_or_insert_with(String::new),
          }
        }

        #[allow(unused)]
        pub fn [<set_ $key>](&mut self, value: impl Into<String>) {
          match self {
            Post::Rss(item) => item.$key = Some(value.into()),
          }
        }
        )*
      }
    }
  };
}

impl_post_get!(title, link, description, author);

impl Post {
  pub fn set_guid(&mut self, value: impl Into<String>) {
    match self {
      Post::Rss(item) => {
        item.guid.get_or_insert_with(Default::default).value = value.into();
      }
    }
  }
}
