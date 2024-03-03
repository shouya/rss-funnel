use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use axum::{routing::get, Extension, Router};
use http::{StatusCode, Uri};
use schemars::schema::RootSchema;
use serde_json::json;

use crate::filter::FilterConfig;
use crate::util::Error;

use super::feed_service::FeedService;

#[derive(rust_embed::RustEmbed)]
#[folder = "inspector/dist/"]
struct Asset;

pub fn router() -> Router {
  Router::new()
    .route("/_inspector/index.html", get(index_handler))
    .route("/_inspector/dist/*file", get(static_handler))
    .route("/_inspector/config", get(config_handler))
    .route("/_inspector/filter_schema", get(filter_schema_handler))
    .route(
      "/",
      get(|| async { Redirect::temporary("/_inspector/index.html") }),
    )
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
  Extension(feed_service): Extension<FeedService>,
) -> impl IntoResponse {
  let json = json!({
    "config_error": feed_service.error(|e| e.to_string()).await,
    "root_config": feed_service.root_config().await,
  });
  Json(json)
}

#[derive(serde::Deserialize)]
struct FilterSchemaHandlerParams {
  filters: String,
}

async fn filter_schema_handler(
  Query(params): Query<FilterSchemaHandlerParams>,
) -> Result<Json<HashMap<String, RootSchema>>, BadRequest<String>> {
  if params.filters == "all" {
    return Ok(Json(FilterConfig::schema_for_all()));
  }

  let mut schemas = HashMap::new();
  for filter in params.filters.split(',') {
    let Some(schema) = FilterConfig::schema_for(filter) else {
      return Err(BadRequest(format!("unknown filter: {}", filter)));
    };
    schemas.insert(filter.to_string(), schema);
  }
  Ok(Json(schemas))
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
struct BadRequest<E>(E);

impl<E: ToString> IntoResponse for BadRequest<E> {
  fn into_response(self) -> Response {
    let body = self.0.to_string();
    http::Response::builder()
      .status(http::StatusCode::BAD_REQUEST)
      .body(body.into())
      .unwrap()
  }
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
