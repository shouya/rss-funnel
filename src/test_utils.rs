use std::{any::Any, str::FromStr, time::Duration};

use http::Request;
use mime::Mime;
use serde::Serialize;
use tower::Service;

use crate::{
  client::{Client, ClientConfig},
  feed::Feed,
  filter::{FeedFilterConfig, FilterConfig},
  server::EndpointConfig,
};

pub fn assert_filter_parse<T>(config: &str, expected: T)
where
  T: FeedFilterConfig + Serialize + 'static,
{
  let parsed: Box<dyn Any> =
    FilterConfig::parse_yaml_variant(config).expect("failed to parse config");

  let actual: Box<T> = parsed
    .downcast()
    .expect("not a filter config of the expected type");

  let actual_serialized = serde_json::to_string(&actual).unwrap();
  let expected_serialized = serde_json::to_string(&expected).unwrap();

  // we must compare the serialized versions because FeedFilterConfig
  // may not be PartialEq. (e.g. Regex is not PartialEq)
  assert_eq!(actual_serialized, expected_serialized);
}

const VALID_CONTENT_TYPES: [&str; 4] = [
  "application/xml",
  "text/xml",
  "application/rss+xml",
  "application/atom+xml",
];

pub async fn fetch_endpoint(config: &str, query: &str) -> Feed {
  let endpoint_config =
    EndpointConfig::parse_yaml(config).expect("failed to parse config");
  let mut endpoint_service = endpoint_config
    .build()
    .await
    .expect("failed to create service")
    .with_client(dummy_client());

  let http_req = Request::get(format!("/endpoint?{query}"))
    .body(axum::body::Body::empty())
    .expect("failed to build request");

  let http_resp = endpoint_service
    .call(http_req)
    .await
    .expect("failed to call service");

  if !http_resp.status().is_success() {
    let status = http_resp.status();
    let body = axum::body::to_bytes(http_resp.into_body(), usize::MAX)
      .await
      .unwrap();
    println!(
      "failed to fetch endpoint: {}",
      std::str::from_utf8(&body).unwrap()
    );
    assert!(status.is_success());
    unreachable!();
  }

  assert!(http_resp.headers().get("content-type").is_some());
  let mime =
    Mime::from_str(http_resp.headers()["content-type"].to_str().unwrap())
      .expect("failed to parse content-type header");
  assert!(VALID_CONTENT_TYPES.contains(&mime.essence_str()));

  let body = axum::body::to_bytes(http_resp.into_body(), usize::MAX)
    .await
    .expect("failed to read body");

  Feed::from_xml_content(&body).expect("failed to parse feed")
}

fn dummy_client() -> Client {
  ClientConfig::default()
    .build(Duration::from_secs(10))
    .expect("failed to build client")
}
