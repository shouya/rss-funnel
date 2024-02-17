use std::sync::Arc;

use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use axum::{routing::get, Extension, Router};
use http::{StatusCode, Uri};
// use maud::{html, Markup};

use crate::config::{self, FeedDefinition};
use crate::util::Error;

#[derive(rust_embed::RustEmbed)]
#[folder = "inspector/dist/"]
#[include = "*.js"]
#[include = "*.css"]
#[include = "*.html"]
// #[include = "*.map"]
struct Asset;

pub fn router(feed_definition: config::FeedDefinition) -> Router {
  Router::new()
    .route("/_inspector/index.html", get(index_handler))
    .route("/_inspector/dist/*file", get(static_handler))
    .route("/_inspector/config", get(config_handler))
    .route(
      "/",
      get(|| async { Redirect::temporary("/_inspector/index.html") }),
    )
    .layer(Extension(Arc::new(feed_definition)))
}

async fn index_handler() -> impl IntoResponse {
  static_handler("/index.html".parse().unwrap()).await
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
  let mut path = uri.path().trim_start_matches('/').to_string();

  if path.starts_with("_inspector/dist/") {
    path = path.replace("_inspector/dist/", "");
  }

  let mime = path.split('.').last().and_then(|ext| match ext {
    "js" => Some([("Content-Type", "application/javascript")]),
    "css" => Some([("Content-Type", "text/css")]),
    "html" => Some([("Content-Type", "text/html")]),
    "map" => Some([("Content-Type", "application/json")]),
    _ => None,
  });

  let content = Asset::get(path.as_str()).map(|content| content.data);

  match (mime, content) {
    (Some(mime), Some(content)) => {
      (StatusCode::OK, mime, content).into_response()
    }
    (None, _) => (
      StatusCode::BAD_REQUEST,
      [("Content-Type", "text/plain")],
      "Invalid file extension",
    )
      .into_response(),
    (_, None) => (
      StatusCode::NOT_FOUND,
      [("Content-Type", "text/plain")],
      "File not found",
    )
      .into_response(),
  }
}

async fn config_handler(
  Extension(feed_definition): Extension<Arc<FeedDefinition>>,
) -> impl IntoResponse {
  Json(feed_definition)
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
