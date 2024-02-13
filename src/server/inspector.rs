use std::sync::Arc;

use axum::response::{IntoResponse, Response};
use axum::{extract::Path, routing::get, Extension, Router};
use maud::{html, Markup, PreEscaped, DOCTYPE};
use tower::Service;

use super::EndpointParam;
use crate::config::{self, FeedDefinition};
use crate::util::{Error, Result};

pub fn router(feed_definition: config::FeedDefinition) -> Router {
  Router::new()
    .route("/", get(main_page))
    .route("/_inspector/preview/:endpoint", get(feed_preview_panel))
    .layer(Extension(Arc::new(feed_definition)))
}

async fn main_page(
  Extension(feed_definition): Extension<Arc<FeedDefinition>>,
) -> Markup {
  html! {
    (DOCTYPE)
    html {
      head {
        meta charset="utf-8";
        title { "RSS Funnel Inspector" }
        style { (PreEscaped(include_str!("../../front/style.css"))) }

        link rel="stylesheet" type="text/css" href="https://cdnjs.cloudflare.com/ajax/libs/codemirror/6.65.7/codemirror.min.css";
        script src="https://cdnjs.cloudflare.com/ajax/libs/codemirror/6.65.7/codemirror.min.js" defer {};
        script src="https://cdnjs.cloudflare.com/ajax/libs/codemirror/6.65.7/mode/xml/xml.min.js" defer {};
        script src="https://cdnjs.cloudflare.com/ajax/libs/codemirror/6.65.7/addon/fold/xml-fold.min.js" defer {};
        link rel="stylesheet" type="text/css" href="https://cdnjs.cloudflare.com/ajax/libs/codemirror/6.65.7/addon/fold/foldgutter.min.css";
        script src="https://cdnjs.cloudflare.com/ajax/libs/codemirror/6.65.7/addon/fold/foldgutter.min.js" defer {};
        script src="https://cdnjs.cloudflare.com/ajax/libs/codemirror/6.65.7/addon/fold/foldcode.min.js" defer {};


      }
      body {
        div {
          (endpoint_selector_panel(&feed_definition))
          (feed_preview_panel_placeholder())
        }
        script { (PreEscaped(include_str!("../../front/inspector.js"))) }
      }
    }
  }
}

pub fn endpoint_selector_panel(feed_definition: &FeedDefinition) -> Markup {
  html! {
    div class="navigation-panel" {
      h4 { "Endpoints" }
      ul class="endpoint-list" {
        @for feed in &feed_definition.endpoints {
          li {
            div class="endpoint" {
              div class="endpoint-path" { (feed.path) }
              @if let Some(note) = &feed.note {
                div class="endpoint-note" { (note) }
              }
            }
          }
        }
      }
    }
  }
}

pub fn feed_preview_panel_placeholder() -> Markup {
  html! {
    div class="feed-preview-panel" {
      div #"feed-preview" {
        "Please select an endpoint."
      }
    }
  }
}

#[axum_macros::debug_handler]
async fn feed_preview_panel(
  Path(endpoint): Path<String>,
  Extension(feed_definition): Extension<Arc<FeedDefinition>>,
) -> Result<String, PreviewError> {
  let endpoint_config = feed_definition
    .endpoints
    .iter()
    .find(|e| e.path.trim_start_matches('/') == endpoint);
  let Some(endpoint_config) = endpoint_config else {
    return Ok("endpoint not found".into());
  };

  let mut service = endpoint_config.clone().into_service().await?;
  let param = EndpointParam::new(None, None, None, true);
  let outcome = service.call(param).await?;

  Ok(outcome.feed_xml().into())
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
struct PreviewError(#[from] Error);

impl IntoResponse for PreviewError {
  fn into_response(self) -> Response {
    let body = self.0.to_string();
    http::Response::builder()
      .status(http::StatusCode::INTERNAL_SERVER_ERROR)
      .body(body.into())
      .unwrap()
  }
}
