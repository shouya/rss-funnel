use maud::{html, Markup, PreEscaped, DOCTYPE};
use url::Url;

use crate::{cli::RootConfig, server::EndpointConfig, source::SourceConfig};

pub fn render_endpoint_list_page(root_config: &RootConfig) -> Markup {
  html! {
    (DOCTYPE)
    head {
      title { "RSS Funnel" }
      meta charset="utf-8";
      (super::header_libs_fragment());
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
      a href={"/_/endpoint/" (endpoint.path.trim_start_matches('/'))} {
        (endpoint.path)
      }

      // badges
      small .ml-1 {
        @if endpoint.config.on_the_fly_filters {
          var .bg-variant .bd-variant .variant title="On-the-fly filters" { "OTF" }
        }

        @let source = endpoint.source();
        (short_source_repr(source))
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
      var .attention.bg-attention.bd-attention { "dynamic" }
    },
    Some(SourceConfig::Simple(url)) if url.starts_with("/") => {
      let path = url_path(url.as_str());
      let path = path.map(|p| format!("/_/{p}"));
      html! {
        @if let Some(path) = path {
          var .accent.bg-accent.bd-accent {
            a href=(path) { "local" }
          }
        } @else {
          var .accent.bg-accent.bd-accent title=(url) {
            "local"
          }
        }
      }
    }
    Some(SourceConfig::Simple(url)) => {
      let host = url_host(url.as_str()).unwrap_or_else(|| "...".into());
      html! {
        var .accent.bg-accent.bd-accent {
          a href=(url) { (host) }
        }
      }
    }
    Some(SourceConfig::FromScratch(_)) => {
      html! {
        var title="Made from scratch" .attention.bg-attention.bd-attention { "scratch" }
      }
    }
    Some(SourceConfig::Templated(_source)) => {
      html! {
        var title="Templated source" .attention.bg-attention.bd-attention { "templated" }
      }
    }
  }
}

fn inline_styles() -> &'static str {
  r#"
  .header-bar {
    margin: 1rem 0 !important;
    padding-bottom: 1rem;
    border-bottom: 1.5px dotted;
    display: flex;
    align-items: center;
    height: 2rem;
  }
  "#
}
