mod filters;

use axum::{
  extract::Path,
  response::{IntoResponse, Response},
  routing, Extension, Router,
};
use duration_str::HumanFormat;
use either::Either;
use http::StatusCode;
use maud::{html, Markup, DOCTYPE};
use serde::Deserialize;

use crate::{
  client::ClientConfig,
  filter::FilterConfig,
  filter_pipeline::{FilterPipeline, FilterPipelineConfig},
  source::{FromScratch, Source, SourceConfig},
};

use super::{
  endpoint::EndpointService, feed_service::FeedService, EndpointConfig,
};

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

  // render config
  let config = render_config_fragment(&endpoint);

  // render config
  let filters =
    render_topmost_filter_pipeline_fragment(&endpoint.config().filters);

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
        section .config-section {
          (config)
        }

        section .filters-section {
          h3 { "Filter pipeline" }
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

fn source_control_fragment(source: &Option<Source>) -> Option<Markup> {
  match source {
    None => Some(html! {
      input type="text" placeholder="Source URL" {}
    }),
    Some(Source::Templated(templated)) => {
      Some(source_template_fragment(templated))
    }
    Some(Source::FromScratch(scratch)) => Some(from_scratch_fragment(scratch)),
    _ => None,
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

fn render_config_fragment(endpoint: &EndpointService) -> Markup {
  let config = endpoint.config();
  let source = endpoint.source();

  html! {
    table {
      tr {
        th { "OTF filters" }
        td {
          @if config.on_the_fly_filters {
            "Enabled"
          } @else {
            "Disabled"
          }
        }
      }
      tr {
        th { "Source" }
        td {
          @if let Some(fragment) = source_control_fragment(source) {
            details {
              summary { (source_summary_fragment(config.source.as_ref())) }
              section title="control" { (fragment) }
            }
          } @else {
            (source_summary_fragment(config.source.as_ref()))
          }
        }
      }

      @if let Some(client) = &config.client {
        tr {
          th { "Client" }
          td { (client_fragment(client)) }
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
      href="https://matcha.mizu.sh/matcha.css"
      referrerpolicy="no-referrer" {}
    style { (maud::PreEscaped(extra_styles())) }
  }
}

fn source_summary_fragment(source: Option<&SourceConfig>) -> Markup {
  html! {
    @match source {
      None => {
        "Dynamic source"
      },
      Some(SourceConfig::Simple(url)) => {
        a .source href=(url) { (url) }
      },
      Some(SourceConfig::FromScratch(_)) => {
        "From scratch"
      },
      Some(SourceConfig::Templated(source)) => {
        span { "Templated source" }
        @if let Some(base) = source.base() {
          span style="margin-left: 10px" { a .source.templated-source href=(base) { (base) } }
        }
      },
    }
  }
}

fn endpoint_list_entry_fragment(endpoint: &EndpointConfig) -> Markup {
  html! {
    section {
      h2 {
        a href={"/_/endpoint/" (endpoint.path.trim_start_matches('/'))} {
          (endpoint.path)
        }
      }

      @if let Some(note) = &endpoint.note {
        section { (note) }
      }

      section {
        @let source = endpoint.source();
        (source_summary_fragment(source))
      }
    }
  }
}

fn client_fragment(client: &ClientConfig) -> Markup {
  html! {
    table {
      @if let Some(user_agent) = &client.user_agent {
        tr {
          th { "User-Agent" }
          td { (user_agent) }
        }
      }

      @if let Some(accept) = &client.accept {
        tr {
          th { "Accept" }
          td { (accept) }
        }
      }

      @if let Some(cookie) = (client.cookie.as_ref()).or(client.set_cookie.as_ref()) {
        tr {
          th { "Cookie" }
          td { (cookie) }
        }
      }

      @if let Some(referer) = &client.referer {
        tr {
          th { "Referer" }
          td { (referer) }
        }
      }

      @if client.accept_invalid_certs {
        tr {
          th { "Accept invalid certs" }
          td { "Yes" }
        }
      }

      @if let Some(proxy) = &client.proxy {
        tr {
          th { "Proxy" }
          td { (proxy) }
        }
      }

      @if let Some(timeout) = &client.timeout {
        tr {
          th { "Timeout" }
          td { (timeout.human_format()) }
        }
      }

      @if let Some(cache_size) = &client.cache_size {
        tr {
          th { "Cache size" }
          td { (cache_size) }
        }
      }

      @if let Some(cache_ttl) = &client.cache_ttl {
        tr {
          th { "Cache TTL" }
          td { (cache_ttl.human_format()) }
        }
      }

      @if let Some(assume_content_type) = &client.assume_content_type {
        tr {
          th { "Assume content type" }
          td { (assume_content_type) }
        }
      }
    }
  }
}

fn render_topmost_filter_pipeline_fragment(
  filters: &FilterPipelineConfig,
) -> Markup {
  render_filter_pipeline_fragment(filters, true, true)
}

fn render_nested_filter_pipeline_fragment(
  filters: &FilterPipelineConfig,
) -> Markup {
  render_filter_pipeline_fragment(filters, false, false)
}

fn render_filter_pipeline_fragment(
  filters: &FilterPipelineConfig,
  render_index: bool,
  render_header: bool,
) -> Markup {
  html! {
    table {
      @if render_header {
        thead {
          tr {
            @if render_index { th { "#" } }
            th { "Type" }
            th { "Config" }
          }
        }
      }
      @for (i, filter) in filters.filters.iter().enumerate() {
        nav {
          @if render_index { th { (i+1) } }
          td { (filter.name()) }
          td {
            @match filter {
              FilterConfig::Js(conf) => (conf),
              FilterConfig::ModifyPost(conf) => (conf),
              FilterConfig::ModifyFeed(conf) => (conf),
              FilterConfig::FullText(_conf) => ("TODO: render filter"),
              FilterConfig::SimplifyHtml(_conf) => ("TODO: render filter"),
              FilterConfig::RemoveElement(_conf) => ("TODO: render filter"),
              FilterConfig::KeepElement(_conf) => ("TODO: render filter"),
              FilterConfig::Split(_conf) => ("TODO: render filter"),
              FilterConfig::Sanitize(_conf) => ("TODO: render filter"),
              FilterConfig::KeepOnly(_conf) => ("TODO: render filter"),
              FilterConfig::Discard(_conf) => ("TODO: render filter"),
              FilterConfig::Highlight(_conf) => ("TODO: render filter"),
              FilterConfig::Merge(conf) => (conf),
              FilterConfig::Note(_conf) => ("TODO: render filter"),
              FilterConfig::ConvertTo(_conf) => ("TODO: render filter"),
              FilterConfig::Limit(_conf) => ("TODO: render filter"),
              FilterConfig::Magnet(_conf) => ("TODO: render filter"),
              FilterConfig::ImageProxy(_conf) => ("TODO: render filter"),
            }
          }
        }
      }
    }
  }
}

fn extra_styles() -> &'static str {
  r#"
  details, ul, ol {
    margin: 0;
  }
"#
}
