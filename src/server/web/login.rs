use axum::{
  extract::FromRequestParts,
  response::{IntoResponse, Redirect, Response},
  Extension, Form,
};
use axum_extra::extract::CookieJar;
use http::request::Parts;
use maud::{html, PreEscaped, DOCTYPE};

use crate::server::feed_service::FeedService;

// Put this in request context
pub struct Auth;

pub async fn handle_login_page() -> impl IntoResponse {
  html! {
    (DOCTYPE);
    head {
      title { "Login - RSS Funnel" }
      meta charset="utf-8";
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
        .map_err(|e| e.into_response())?;

    if !feed_service.requires_auth().await {
      return Ok(Auth);
    }

    let cookie_jar = CookieJar::from_request_parts(parts, state)
      .await
      .map_err(|e| e.into_response())?;

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
  Redirect::to("/_/login?login_required=1").into_response()
}

pub async fn handle_logout(cookie_jar: CookieJar) -> impl IntoResponse {
  let cookie_jar = cookie_jar.remove("session_id");
  (cookie_jar, Redirect::to("/_/login?logged_out=1")).into_response()
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
  match feed_service.login(&params.username, &params.password).await {
    Some(session_id) => {
      let cookie_jar = cookie_jar.add(("session_id", session_id));
      (cookie_jar, Redirect::to("/_/endpoints")).into_response()
    }
    _ => Redirect::to("/_/login?bad_auth=1").into_response(),
  }
}

fn inline_styles() -> &'static str {
  r#"
  body {
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  form {
    margin-top: 15px;
    margin-bottom: 20px;
    display: flex;
    flex-direction: column;
    width: 200px;
    gap: 0.5rem;
  }

   .hidden {
     display: none;
   }

   p#message {
     color: red;
   }
   "#
}

fn inline_scripts() -> &'static str {
  r#"
  function setErrorMessage() {
    const message = document.getElementById("message");
    const search = window.location.search;
    if (search.includes("bad_auth=1")) {
      message.classList.remove("hidden");
      message.textContent = "Invalid username or password";
    } else if (search.includes("logged_out=1")) {
      message.classList.remove("hidden");
      message.textContent = "You have been logged out";
    } else if (search.includes("login_required=1")) {
      message.classList.remove("hidden");
      message.textContent = "You must be logged in to access that page";
    }
  }
"#
}
