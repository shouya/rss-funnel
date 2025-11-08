use axum::{
  Extension, Form,
  extract::FromRequestParts,
  response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::CookieJar;
use http::request::Parts;

use crate::util::relative_path;

use super::feed_service::FeedService;

pub struct Auth;

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

    let session_id = cookie_jar.get("session_id").ok_or_else(login)?.value();

    if !feed_service.validate_session_id(session_id).await {
      return Err(login());
    }

    Ok(Auth)
  }
}

fn login() -> Response {
  let login_path = relative_path("_inspector/login.html?login_required=1");
  Redirect::to(&login_path).into_response()
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
  if let Some(session_id) =
    feed_service.login(&params.username, &params.password).await
  {
    let cookie_jar = cookie_jar.add(("session_id", session_id));
    let home_path = relative_path("_inspector/index.html");
    (cookie_jar, Redirect::to(&home_path)).into_response()
  } else {
    let login_path = relative_path("_inspector/login.html?bad_auth=1");
    Redirect::to(&login_path).into_response()
  }
}

pub async fn handle_logout(cookie_jar: CookieJar) -> Response {
  let cookie_jar = cookie_jar.remove("session_id");
  let logout_path = relative_path("_inspector/login.html?logged_out=1");
  (cookie_jar, Redirect::to(&logout_path)).into_response()
}
