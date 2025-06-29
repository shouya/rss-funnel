use std::borrow::Cow;

use maud::{html, Markup, PreEscaped, DOCTYPE};
use url::Url;

use crate::{
  cli::RootConfig,
  server::{web::sprite, EndpointConfig},
  source::SourceConfig,
  util::relative_path,
};

pub fn render_endpoint_list_page(
  root_config: &RootConfig,
  // (style, message)
  reload_message: Option<(&str, String)>,
) -> Markup {
  html! {
    (DOCTYPE)
    head {
      title { "RSS Funnel" }
      meta charset="utf-8";
      (super::favicon());
      (super::header_libs_fragment());
      style { (PreEscaped(super::common_styles())) }
      style { (PreEscaped(inline_styles())) }
    }
    body {
      header .header-bar {
        h2 { "RSS Funnel" }

        button .button
          name="reload"
          hx-get=(format!("/_/endpoints"))
          hx-target="main"
          hx-select="main"
          title="Reload config file" {
            (sprite("reload"))
          }
      }

      main {
        @if let Some((style, message)) = reload_message {
          section .flash.(style) style="margin: 1rem;" {
            (message)
          }
        }

        ul {
          @for endpoint in &root_config.endpoints {
            (endpoint_list_entry_fragment(endpoint))
          }
        }
      }
    }
  }
}

fn endpoint_list_entry_fragment(endpoint: &EndpointConfig) -> Markup {
  html! {
    li {
      p {
        @let normalized_path = endpoint.path.trim_start_matches('/');
        @let endpoint_path = format!("_/endpoint/{normalized_path}");
        @let endpoint_path = relative_path(&endpoint_path);
        a href=(endpoint_path) {
          (endpoint.path)
        }

        // badges
        span .tag-container {
          @if endpoint.config.on_the_fly_filters {
            span .tag.otf  title="On-the-fly filters" { "OTF" }
          }

          @let source = endpoint.source();
          (short_source_repr(source))
        }
      }

      @if let Some(note) = &endpoint.note {
        p { (note) }
      }
    }
  }
}

fn url_host(url: impl TryInto<Url>) -> Option<String> {
  let Ok(url) = url.try_into() else {
    return None;
  };

  url.host_str().map(std::borrow::ToOwned::to_owned)
}

fn url_path(url: impl TryInto<Url>) -> Option<String> {
  let Ok(url) = url.try_into() else {
    return None;
  };

  Some(url.path().to_owned())
}

fn short_source_repr(source: &SourceConfig) -> Markup {
  match source {
    SourceConfig::Dynamic => html! {
      span .tag.dynamic { "dynamic" }
    },
    SourceConfig::Simple(url) if url.starts_with('/') => {
      let path = url_path(url.as_str());
      let path = path.map(|p| relative_path(&format!("_/{p})")));
      html! {
        @if let Some(path) = path {
          span .tag.local {
            a href=(path) { "local" }
          }
        } @else {
          span .tag.local title=(url) {
            "local"
          }
        }
      }
    }
    SourceConfig::Simple(url) => {
      let host = url_host(url.as_str()).unwrap_or_else(|| "...".into());
      html! {
        span .tag.simple {
          a href=(url) { (host) }
        }
      }
    }
    SourceConfig::FromScratch(_) => {
      html! {
        span .tag.scratch title="Made from scratch" { "scratch" }
      }
    }
    SourceConfig::Templated(_source) => {
      html! {
        span .tag.templated title="Templated source" { "templated" }
      }
    }
  }
}

fn inline_styles() -> Cow<'static, str> {
  super::Asset::get_content("list.css")
}
