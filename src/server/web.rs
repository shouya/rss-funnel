mod endpoint;
mod list;
mod login;

use std::{borrow::Cow, convert::Infallible};

use axum::{
  extract::Path,
  response::{IntoResponse, Redirect, Response},
  routing, Extension, Router,
};
use http::StatusCode;
use login::Auth;
use maud::{html, Markup};

use crate::util::relative_path;

use super::{feed_service::FeedService, EndpointParam};

#[derive(rust_embed::RustEmbed)]
#[folder = "static/"]
#[include = "*.js"]
#[include = "*.css"]
struct Asset;

impl Asset {
  fn get_content(name: &str) -> Cow<'static, str> {
    let file = <Asset as rust_embed::RustEmbed>::get(name).unwrap();
    match file.data {
      Cow::Borrowed(data) => String::from_utf8_lossy(data),
      Cow::Owned(data) => String::from_utf8_lossy(&data).into_owned().into(),
    }
  }
}

pub fn router() -> Router {
  Router::new()
    .route("/", routing::get(handle_home))
    .route(
      "/login",
      routing::get(login::handle_login_page).post(login::handle_login),
    )
    .route("/logout", routing::get(login::handle_logout))
    // requires login
    .route("/endpoints", routing::get(handle_endpoint_list))
    .route("/endpoint/:path", routing::get(handle_endpoint))
    .route("/sprite.svg", routing::get(handle_sprite))
}

async fn handle_sprite() -> impl IntoResponse {
  let svg = include_str!("../../static/sprite.svg");
  (StatusCode::OK, [("Content-Type", "image/svg+xml")], svg)
}

// bool: whether the reload is successful
struct AutoReload(Option<bool>);

impl AutoReload {
  // (style, message)
  async fn flash_message(
    &self,
    service: &FeedService,
  ) -> Option<(&'static str, String)> {
    match self.0 {
      None => None,
      Some(true) => Some(("success", "Config reloaded.".to_owned())),
      Some(false) => service.with_error(|e| ("error", e.to_string())).await,
    }
  }
}

#[async_trait::async_trait]
impl<S> axum::extract::FromRequestParts<S> for AutoReload {
  type Rejection = Infallible;

  async fn from_request_parts(
    parts: &mut http::request::Parts,
    _state: &S,
  ) -> Result<Self, Self::Rejection> {
    let Some(value) = parts.headers.get("HX-Trigger-Name") else {
      return Ok(Self(None));
    };

    if value.as_bytes() != b"reload" {
      return Ok(Self(None));
    }

    let Some(feed_service) = parts.extensions.get::<FeedService>() else {
      return Ok(Self(None));
    };

    Ok(Self(Some(feed_service.reload().await)))
  }
}

async fn handle_home(auth: Option<Auth>) -> impl IntoResponse {
  if auth.is_some() {
    let endpoints_path = relative_path("_/endpoints");
    Redirect::temporary(&endpoints_path)
  } else {
    let login_path = relative_path("_/login");
    Redirect::temporary(&login_path)
  }
}

async fn handle_endpoint_list(
  _: Auth,
  auto_reload: AutoReload,
  Extension(service): Extension<FeedService>,
) -> Markup {
  let root_config = service.root_config().await;

  let reload_message = auto_reload.flash_message(&service).await;
  list::render_endpoint_list_page(&root_config, reload_message)
}

async fn handle_endpoint(
  _: Auth,
  Path(path): Path<String>,
  auto_reload: AutoReload,
  Extension(service): Extension<FeedService>,
  param: EndpointParam,
) -> Result<Markup, Response> {
  let endpoint = service.get_endpoint(&path).await.ok_or_else(|| {
    (StatusCode::NOT_FOUND, format!("Endpoint {path} not found"))
      .into_response()
  })?;

  let reload_message = auto_reload.flash_message(&service).await;
  let page =
    endpoint::render_endpoint_page(endpoint, path, param, reload_message).await;

  Ok(page)
}

fn header_libs_fragment() -> Markup {
  html! {
    script
      src="https://unpkg.com/htmx.org@2.0.1/dist/htmx.min.js"
      referrerpolicy="no-referrer" {}
  }
}

fn favicon() -> Markup {
  html! {
    link
      rel="icon"
      type="image/svg+xml"
      href="data:image/svg+xml,%3Csvg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-45 -50 140 140\"%3E%3Ccircle cx=\"306.7\" cy=\"343.7\" r=\"11.4\" fill=\"%23ff7b00\" transform=\"translate(-282 -331)\"/%3E%3Cpath fill=\"none\" stroke=\"%23ff7b00\" stroke-width=\"15\" d=\"M-3 16a29 29 0 1 1 56 0\"/%3E%3Cpath fill=\"none\" stroke=\"%23ff7b00\" stroke-width=\"15\" d=\"M-23 18a49 49 0 1 1 96-1\"/%3E%3Cpath fill=\"%23ff7b00\" d=\"m-24 29 98-1-1 10-40 28 1 19H17l1-20-42-27z\"/%3E%3C/svg%3E%0A";
  }
}

fn sprite(icon: &str) -> Markup {
  let sprite_path = relative_path("_/sprite.svg");
  html! {
    svg class="icon" xmlns="http://www.w3.org/2000/svg" width="20" height="20" {
      use xlink:href=(format!("{sprite_path}#{icon}"));
    }
  }
}

fn common_styles() -> Cow<'static, str> {
  Asset::get_content("common.css")
}
