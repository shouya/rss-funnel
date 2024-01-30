use std::any::Any;

use serde::Serialize;

use crate::filter::{FeedFilterConfig, FilterConfig};

pub fn assert_filter_parse<T>(config: &str, expected: T)
where
  T: FeedFilterConfig + Serialize + 'static,
{
  let parsed: Box<dyn Any> =
    FilterConfig::parse_yaml(config).expect("failed to parse config");

  let actual: Box<T> = parsed
    .downcast()
    .expect("not a filter config of the expected type");

  let actual_serialized = serde_json::to_string(&actual).unwrap();
  let expected_serialized = serde_json::to_string(&expected).unwrap();

  // we must compare the serialized versions because FeedFilterConfig
  // may not be PartialEq. (e.g. Regex is not PartialEq)
  assert_eq!(actual_serialized, expected_serialized);
}
