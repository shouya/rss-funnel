use std::collections::HashMap;

use rquickjs::{class::Trace, function::Opt, Ctx, Exception};
use serde::{Deserialize, Serialize};

use super::AsJson;

#[derive(Deserialize)]
#[allow(clippy::upper_case_acronyms)]
enum Method {
  GET,
  POST,
  PUT,
  DELETE,
}

impl From<Method> for reqwest::Method {
  fn from(method: Method) -> Self {
    match method {
      Method::GET => reqwest::Method::GET,
      Method::POST => reqwest::Method::POST,
      Method::PUT => reqwest::Method::PUT,
      Method::DELETE => reqwest::Method::DELETE,
    }
  }
}

#[derive(Deserialize)]
pub(super) struct RequestParams {
  method: Method,
  headers: HashMap<String, String>,
  body: Option<String>,
}

impl Default for RequestParams {
  fn default() -> Self {
    Self {
      method: Method::GET,
      headers: HashMap::new(),
      body: None,
    }
  }
}

#[derive(Trace, Serialize)]
#[rquickjs::class]
pub(super) struct Response {
  status: u16,
  headers: HashMap<String, String>,
  body: String,
}

pub(super) async fn fetch(
  ctx: Ctx<'_>,
  url: String,
  params: Opt<AsJson<RequestParams>>,
) -> Result<Response, rquickjs::Error> {
  let params = params.into_inner().map(|x| x.0).unwrap_or_default();
  let client = reqwest::Client::new();
  let mut builder = client.request(params.method.into(), url);

  for (k, v) in params.headers {
    builder = builder.header(k, v);
  }

  if let Some(body) = params.body {
    builder = builder.body(body);
  }

  let resp = builder
    .send()
    .await
    .map_err(|e| Exception::throw_message(&ctx, &e.to_string()))?;
  let status = resp.status().as_u16();
  let mut headers = HashMap::new();

  for (k, v) in resp.headers() {
    headers.insert(k.as_str().to_string(), v.to_str().unwrap().to_string());
  }

  let body = resp
    .text()
    .await
    .map_err(|e| Exception::throw_message(&ctx, &e.to_string()))?;

  Ok(Response {
    status,
    headers,
    body,
  })
}
