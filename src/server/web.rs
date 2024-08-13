use axum::{
  extract::Path,
  response::{IntoResponse, Response},
  routing, Extension, Router,
};
use either::Either;
use http::StatusCode;
use maud::{html, Markup, PreEscaped, DOCTYPE};
use serde::Deserialize;

use crate::source::{FromScratch, Source, SourceConfig};

use super::{feed_service::FeedService, EndpointConfig};

pub fn router() -> Router {
  Router::new()
    .route("/", routing::get(handle_home))
    .route("/endpoint/:path", routing::get(handle_endpoint))
}

async fn handle_home(Extension(service): Extension<FeedService>) -> Markup {
  let root_config = service.root_config().await;

  html! {
    (DOCTYPE)
    head {
      title { "RSS Funnel" }
      meta charset="utf-8";
      (header_libs_fragment())
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

#[derive(Deserialize)]
struct HandleEndpointQuery {
  raw: Option<bool>,
}

async fn handle_endpoint(
  Path(path): Path<String>,
  Extension(service): Extension<FeedService>,
) -> Result<Markup, Response> {
  let endpoint = service.get_endpoint(&path).await.ok_or_else(|| {
    (StatusCode::NOT_FOUND, format!("Endpoint {path} not found"))
      .into_response()
  })?;

  // render source control
  let source = source_control_fragment(endpoint.source());

  // render config
  let config = "TODO: render config";

  // render config
  let filters = "TODO: render filters";

  // render feed preview
  let feed = "TODO: render feed";

  let markup = html! {
    (DOCTYPE)
    head {
      title { "RSS Funnel" }
      meta charset="utf-8";
      (header_libs_fragment())
    }
    body {
      h1 { "RSS Funnel" }
      main {
        h2 { (path) }
        section .source-section {
          h3 { "Source" }
          (source)
        }
        section .config-section {
          h3 { "Config" }
          (config)
        }

        aside .filters-section {
          h3 { "Filters" }
          (filters)
        }

        section .feed-section {
          h3 { "Feed" }
          (feed)
        }
      }
    }
  };

  Ok(markup)
}

fn source_control_fragment(source: &Option<Source>) -> Markup {
  match source {
    None => html! {
      input type="text" placeholder="Source URL" {}
    },
    Some(Source::AbsoluteUrl(url)) => html! {
      a .source href=(url) { (url) }
    },
    Some(Source::RelativeUrl(url)) => html! {
      a .source href=(url) { (url) }
    },
    Some(Source::Templated(templated)) => source_template_fragment(templated),
    Some(Source::FromScratch(scratch)) => from_scratch_fragment(scratch),
  }
}

fn from_scratch_fragment(scratch: &FromScratch) -> Markup {
  html! {
    table {
      tbody {
        tr {
          th { "Format" }
          td { (scratch.format.as_str()) }
        }
        tr {
          th { "Title" }
          td { (scratch.title) }
        }
        @if let Some(link) = &scratch.link {
          tr {
            th { "Link" }
            td { (link) }
          }
        }
        @if let Some(description) = &scratch.description {
          tr {
            th { "Description" }
            td { (description) }
          }
        }
      }
    }
  }
}

fn source_template_fragment(templated: &crate::source::Templated) -> Markup {
  html! {
    @for fragment in templated.fragments() {
      @match fragment {
        Either::Left(plain) => span { (plain) },
        Either::Right((name, Some(placeholder))) => {
          @let value=placeholder.default.as_ref();
          @let validation=placeholder.validation.as_ref();
          input
            id={"placeholder-" (name)}
            name=(name)
            placeholder=(name)
            pattern=[validation]
            value=[value]
          {}
        }
        Either::Right((name, None)) => {
          span style="color: red" title="Placeholder not defined" { "${" (name) "}" }
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
        a .source href=(url) { (url) }
      },
      SourceConfig::FromScratch(_) => {
        p { "From scratch" }
      },
      SourceConfig::Templated(source) => {
        @if let Some(base) = source.base() {
          a .source.templated-source href=(base) { (base) }
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
