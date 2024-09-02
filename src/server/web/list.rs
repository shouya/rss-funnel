use maud::{html, Markup, DOCTYPE};
use url::Url;

use crate::{cli::RootConfig, server::EndpointConfig, source::SourceConfig};

pub fn render_endpoint_list_page(root_config: &RootConfig) -> Markup {
  html! {
    (DOCTYPE)
    head {
      title { "RSS Funnel" }
      meta charset="utf-8";
      (super::header_libs_fragment())
    }
    body {
      h1 { "RSS Funnel" }
      main {
        @for endpoint in &root_config.endpoints {
          (endpoint_list_entry_fragment(endpoint))
        }
      }
    }
  }
}

fn endpoint_list_entry_fragment(endpoint: &EndpointConfig) -> Markup {
  html! {
    hgroup {
      a href={"/_/endpoint/" (endpoint.path.trim_start_matches('/'))} {
        (endpoint.path)
      }

      @let source = endpoint.source();
      sub { q { (source_summary_fragment(source)) } }

      @if let Some(note) = &endpoint.note {
        section { (note) }
      }
    }
  }
}

fn source_summary_fragment(source: Option<&SourceConfig>) -> Markup {
  html! {
    @match source {
      None => {
        "dynamic"
      },
      Some(SourceConfig::Simple(url)) => {
        @if let Some(host) = url_host(url.as_str()) {
          a .source href=(url) { (host) }
        } @else {
          a .source href=(url) { "..." }
        }
      },
      Some(SourceConfig::FromScratch(_)) => {
        "scratch"
      },
      Some(SourceConfig::Templated(source)) => {
        @if let Some(host) = url_host(source.template.as_str()) {
          (host)

        } else {
          "templated"
        }
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

fn short_source_repr(
  source: Option<&SourceConfig>,
) -> (String, Option<String>) {
  match source {
    None => ("dynamic".to_owned(), None),
    Some(SourceConfig::Simple(url)) if url.starts_with("/") => {
      let path = url_path(url.as_str());
      ("local".to_owned(), path.map(|p| format!("/_/{p}")))
    }
    Some(SourceConfig::Simple(url)) => {
      let host = url_host(url.as_str()).unwrap_or_else(|| "...".into());
      (host, Some(url.clone()))
    }
    Some(SourceConfig::FromScratch(_)) => ("scratch".to_owned(), None),
    Some(SourceConfig::Templated(source)) => ("templated".to_owned(), None),
  }
}
