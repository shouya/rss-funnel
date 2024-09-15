use std::borrow::Cow;

use maud::{html, Markup, PreEscaped, DOCTYPE};
use url::Url;

use crate::{cli::RootConfig, server::EndpointConfig, source::SourceConfig};

pub fn render_endpoint_list_page(root_config: &RootConfig) -> Markup {
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
      }

      main {
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
    li ."my-.5" {
      p {
        a href={"/_/endpoint/" (endpoint.path.trim_start_matches('/'))} {
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

  url.host_str().map(|s| s.to_owned())
}

fn url_path(url: impl TryInto<Url>) -> Option<String> {
  let Ok(url) = url.try_into() else {
    return None;
  };

  Some(url.path().to_owned())
}

fn short_source_repr(source: Option<&SourceConfig>) -> Markup {
  match source {
    None => html! {
      span .tag.dynamic { "dynamic" }
    },
    Some(SourceConfig::Simple(url)) if url.starts_with("/") => {
      let path = url_path(url.as_str());
      let path = path.map(|p| format!("/_/{p}"));
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
    Some(SourceConfig::Simple(url)) => {
      let host = url_host(url.as_str()).unwrap_or_else(|| "...".into());
      html! {
        span .tag.simple {
          a href=(url) { (host) }
        }
      }
    }
    Some(SourceConfig::FromScratch(_)) => {
      html! {
        span .tag.scratch title="Made from scratch" { "scratch" }
      }
    }
    Some(SourceConfig::Templated(_source)) => {
      html! {
        span .tag.templated title="Templated source" { "templated" }
      }
    }
  }
}

fn inline_styles() -> Cow<'static, str> {
  super::Asset::get_content("list.css")
}
