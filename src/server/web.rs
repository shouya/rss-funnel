use axum::{routing, Extension, Router};
use maud::{html, Markup};

use crate::source::SourceConfig;

use super::{feed_service::FeedService, EndpointConfig};

pub fn router() -> Router {
  Router::new().route("/", routing::get(home))
}

async fn home(Extension(service): Extension<FeedService>) -> Markup {
  let root_config = service.root_config().await;

  html! {
    head {
      title { "RSS Funnel" }
      meta charset="utf-8";
      (header_libs_fragment())
    }
    body {
      h1 { "RSS Funnel" }
      hr {}
      main {
        @for endpoint in &root_config.endpoints {
          (endpoint_list_entry_fragment(endpoint))
        }
      }
    }
  }
}

fn header_libs_fragment() -> Markup {
  html! {
    script
      src="https://unpkg.com/htmx.org@2.0.1"
      referrerpolicy="no-referrer" {}
    link
      rel="stylesheet"
      href="https://unpkg.com/bamboo.css"
      referrerpolicy="no-referrer" {}
  }
}

fn source_summary_fragment(source: &SourceConfig) -> Markup {
  html! {
    @match source {
      SourceConfig::Simple(url) => {
        a class="source" href=(url) { (url) }
      },
      SourceConfig::FromScratch(_) => {
        p { "From scratch" }
      },
      SourceConfig::Templated(source) => {
        @if let Some(base) = source.base() {
          a class="source templated-source" href=(base) { (base) }
        } else {
          p { "Templated source" }
        }
      },
    }
  }
}

fn endpoint_list_entry_fragment(endpoint: &EndpointConfig) -> Markup {
  html! {
    section {
      @if let Some(note) = &endpoint.note {
        aside { (note) }
      }

      h2 {
        a href={"/_/endpoint/" (endpoint.path.trim_start_matches('/'))} {
          (endpoint.path)
        }
      }

      article {
        @if let Some(source) = &endpoint.source() {
          (source_summary_fragment(source))
        }
      }
    }
  }
}
