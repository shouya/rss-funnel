use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum::{
  routing::{get, post},
  Extension, Router,
};
use http::{StatusCode, Uri};
use schemars::schema::RootSchema;
use serde_json::json;

use crate::filter::FilterConfig;
use crate::util::Error;

use super::auth::{handle_login, handle_logout, Auth};
use super::feed_service::FeedService;
use super::EndpointParam;

#[derive(rust_embed::RustEmbed)]
#[folder = "inspector/dist/"]
struct Asset;

pub fn router() -> Router {
  Router::new()
    .route("/login", post(handle_login))
    .route("/logout", get(handle_logout))
    .route("/_inspector/index.html", get(inspector_page_handler))
    .route("/_inspector/login.html", get(login_page_handler))
    .route("/_inspector/dist/*file", get(static_handler))
    .route("/_inspector/config", get(config_handler))
    .route("/_inspector/filter_schema", get(filter_schema_handler))
    .route("/_inspector/preview", get(preview_handler))
}

async fn inspector_page_handler(_auth: Auth) -> impl IntoResponse {
  static_handler("/index.html".parse().unwrap()).await
}

async fn login_page_handler() -> impl IntoResponse {
  static_handler("/login.html".parse().unwrap()).await
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
  _auth: Auth,
) -> impl IntoResponse {
  let json = json!({
    "config_error": feed_service.with_error(|e| e.to_string()).await,
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
  _auth: Auth,
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

#[derive(serde::Deserialize)]
struct PreviewHandlerParams {
  endpoint: String,
}

async fn preview_handler(
  Extension(feed_service): Extension<FeedService>,
  endpoint_param: EndpointParam,
  Query(params): Query<PreviewHandlerParams>,
  _auth: Auth,
) -> Result<impl IntoResponse, PreviewError> {
  let path = params.endpoint.trim_start_matches('/');
  let endpoint_service = feed_service.get_endpoint(path).await;
  let Some(endpoint_service) = endpoint_service else {
    let e = Error::EndpointNotFound(params.endpoint);
    return Err(PreviewError(e));
  };

  let feed = endpoint_service.run(endpoint_param).await?;
  let body = json!({
    "content_type": feed.content_type(),
    "post_count": feed.post_count(),
    "unified": feed.preview(),
    "raw": feed.serialize(true)?,
    "json": feed
  });
  Ok(Json(body))
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
