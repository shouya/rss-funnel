mod endpoint;
mod filters;
mod list;

use axum::{
  extract::Path,
  response::{IntoResponse, Response},
  routing, Extension, Router,
};
use http::StatusCode;
use maud::{html, Markup};

use super::feed_service::FeedService;

pub fn router() -> Router {
  Router::new()
    .route("/", routing::get(handle_home))
    .route("/endpoint/:path", routing::get(handle_endpoint))
}

async fn handle_home(Extension(service): Extension<FeedService>) -> Markup {
  let root_config = service.root_config().await;
  list::render_endpoint_list_page(&root_config)
}

async fn handle_endpoint(
  Path(path): Path<String>,
  Extension(service): Extension<FeedService>,
) -> Result<Markup, Response> {
  let endpoint = service.get_endpoint(&path).await.ok_or_else(|| {
    (StatusCode::NOT_FOUND, format!("Endpoint {path} not found"))
      .into_response()
  })?;

  Ok(endpoint::render_endpoint_page(&endpoint))
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

fn extra_styles() -> &'static str {
  r#"
  details, ul, ol {
    margin: 0;
  }
"#
}
