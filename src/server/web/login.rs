use std::borrow::Cow;

use axum::{
  extract::FromRequestParts,
  response::{IntoResponse, Redirect, Response},
  Extension, Form,
};
use axum_extra::extract::CookieJar;
use http::request::Parts;
use maud::{html, PreEscaped, DOCTYPE};

use crate::{server::feed_service::FeedService, util::relative_path};

// Put this in request context
pub struct Auth;

pub async fn handle_login_page() -> impl IntoResponse {
  html! {
    (DOCTYPE);
    head {
      title { "Login - RSS Funnel" }
      meta charset="utf-8";
      (super::favicon());
      (super::header_libs_fragment());
      style { (PreEscaped(inline_styles())) }
      script { (PreEscaped(inline_scripts())) }
    }

    body onload="setErrorMessage()" {
      p #message .hidden {}

      form method="post" {
        input type="text" name="username" placeholder="Username";
        input type="password" name="password" placeholder="Password";
        button type="submit" { "Login" }
      }
      footer {
        p {
          "Powered by ";
          a href="https://github.com/shouya/rss-funnel" { "RSS Funnel" }
        }
      }
    }
  }
}

#[async_trait::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for Auth {
  type Rejection = Response;

  async fn from_request_parts(
    parts: &mut Parts,
    state: &S,
  ) -> Result<Self, Self::Rejection> {
    let feed_service: Extension<FeedService> =
      Extension::from_request_parts(parts, state)
        .await
        .map_err(axum::response::IntoResponse::into_response)?;

    if !feed_service.requires_auth().await {
      return Ok(Auth);
    }

    let cookie_jar = CookieJar::from_request_parts(parts, state)
      .await
      .map_err(axum::response::IntoResponse::into_response)?;

    let session_id = cookie_jar
      .get("session_id")
      .ok_or_else(redir_login)?
      .value();

    if !feed_service.validate_session_id(session_id).await {
      return Err(redir_login());
    }

    Ok(Auth)
  }
}

fn redir_login() -> Response {
  let login_path = relative_path("_/login?login_required=1");
  Redirect::to(&login_path).into_response()
}

pub async fn handle_logout(cookie_jar: CookieJar) -> impl IntoResponse {
  let cookie_jar = cookie_jar.remove("session_id");
  let login_path = relative_path("_/login?logged_out=1");
  (cookie_jar, Redirect::to(&login_path)).into_response()
}

#[derive(serde::Deserialize)]
pub struct HandleLoginParams {
  username: String,
  password: String,
}

pub async fn handle_login(
  cookie_jar: CookieJar,
  Extension(feed_service): Extension<FeedService>,
  Form(params): Form<HandleLoginParams>,
) -> Response {
  let cookie_jar = cookie_jar.remove("session_id");
  if let Some(session_id) =
    feed_service.login(&params.username, &params.password).await
  {
    let cookie_jar = cookie_jar.add(("session_id", session_id));
    let home_path = relative_path("_/");
    (cookie_jar, Redirect::to(&home_path)).into_response()
  } else {
    let login_path = relative_path("_/login?bad_auth=1");
    Redirect::to(&login_path).into_response()
  }
}

fn inline_styles() -> Cow<'static, str> {
  super::Asset::get_content("login.css")
}

fn inline_scripts() -> Cow<'static, str> {
  super::Asset::get_content("login.js")
}
