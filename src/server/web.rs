mod endpoint;
mod list;

use axum::{
  extract::{rejection::QueryRejection, Path, Query},
  response::{IntoResponse, Response},
  routing, Extension, Router,
};
use http::StatusCode;
use maud::{html, Markup};

use super::{feed_service::FeedService, EndpointParam};

pub fn router() -> Router {
  Router::new()
    .route("/", routing::get(handle_home))
    .route("/endpoint/:path", routing::get(handle_endpoint))
    .route("/sprite.svg", routing::get(handle_sprite))
}

async fn handle_sprite() -> impl IntoResponse {
  let svg = include_str!("../../static/sprite.svg");
  (StatusCode::OK, [("Content-Type", "image/svg+xml")], svg)
}

async fn handle_home(Extension(service): Extension<FeedService>) -> Markup {
  let root_config = service.root_config().await;
  list::render_endpoint_list_page(&root_config)
}

async fn handle_endpoint(
  Path(path): Path<String>,
  Extension(service): Extension<FeedService>,
  param: Result<Query<EndpointParam>, QueryRejection>,
) -> Result<Markup, Response> {
  let endpoint = service.get_endpoint(&path).await.ok_or_else(|| {
    (StatusCode::NOT_FOUND, format!("Endpoint {path} not found"))
      .into_response()
  })?;

  let param = param.map(|q| q.0).map_err(|e| e.body_text());
  Ok(endpoint::render_endpoint_page(endpoint, path, param).await)
}

fn header_libs_fragment() -> Markup {
  html! {
    script
      src="https://unpkg.com/htmx.org@2.0.2"
      referrerpolicy="no-referrer" {}
    link
      rel="stylesheet"
      href="https://matcha.mizu.sh/matcha.css"
      referrerpolicy="no-referrer";
    style { (maud::PreEscaped(extra_styles())) }
  }
}

fn extra_styles() -> &'static str {
  r#""#
}

pub fn sprite(icon: &str) -> Markup {
  html! {
    svg class="icon" xmlns="http://www.w3.org/2000/svg" width="20" height="20" {
      use xlink:href=(format!("/_/sprite.svg#{icon}"));
    }
  }
}
