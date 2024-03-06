use std::collections::HashMap;

use rquickjs::{class::Trace, function::Opt, Ctx, Exception};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Trace, Serialize, Debug)]
#[rquickjs::class]
pub(super) struct Response {
  #[qjs(get)]
  status: u16,
  #[qjs(get)]
  headers: HashMap<String, String>,
  #[qjs(get)]
  body: String,
}

#[rquickjs::methods]
impl Response {
  fn json(&self, ctx: Ctx<'_>) -> Result<AsJson<Value>, rquickjs::Error> {
    if self
      .content_type()
      .filter(|v| v.starts_with("application/json"))
      .is_none()
    {
      return Err(Exception::throw_message(&ctx, "response is not JSON"));
    }

    let json: serde_json::Value = match serde_json::from_str(&self.body) {
      Ok(json) => json,
      Err(e) => return Err(Exception::throw_message(&ctx, &e.to_string())),
    };

    Ok(AsJson(json))
  }

  fn content_type(&self) -> Option<String> {
    if let Some(content_type) = self.headers.get("content-type") {
      return Some(content_type.to_string());
    };

    if let Some(content_type) = self.headers.get("Content-Type") {
      return Some(content_type.to_string());
    }

    self
      .headers
      .iter()
      .filter_map(|(k, v)| {
        if k.to_ascii_lowercase() == "content-type" {
          Some(v)
        } else {
          None
        }
      })
      .next()
      .cloned()
  }
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

#[cfg(test)]
mod tests {
  use crate::test_utils::fetch_endpoint;
  use crate::util::Result;

  #[tokio::test]
  async fn test_fetch() -> Result<()> {
    let config = r#"
      !endpoint
      path: /fetch
      source: fixture:///minimal_rss_20.xml
      filters:
        - modify_post: |
            let x = await fetch("http://example.com");
            console.log(JSON.stringify(x));
            post.description = x.body;
    "#;

    let _feed = fetch_endpoint(config, "").await;

    Ok(())
  }
}
