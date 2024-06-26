use std::str::FromStr;

use tracing::warn;

use crate::{
  filter::FilterConfig, filter_pipeline::FilterPipeline, util::ConfigError,
};

struct FilterPipelineCache {
  query: Vec<String>,
  pipeline: Option<FilterPipeline>,
}

struct InlineFilter {
  cache: FilterPipelineCache,
}

impl InlineFilter {}

fn parse_config(param: &str) -> Result<FilterConfig, ConfigError> {
  use serde_yaml::{Mapping, Number, Value};
  if !param.contains('=') {
    let value = Mapping::new().into();
    return Ok(FilterConfig::parse_yaml_value(param, value)?);
  }

  let Some((name, value)) = param.split_once('=') else {
    warn!("invalid inline param: {}", param);
    let message = format!("invalid inline param: {}", param);
    return Err(ConfigError::Message(message));
  };

  // try parse value as number if possible
  if let Ok(num) = Number::from_str(value) {
    let value = Value::Number(num);
    if let Ok(config) = FilterConfig::parse_yaml_value(name, value) {
      return Ok(config);
    }
  }

  // try parse value as string
  let value = Value::String(value.to_string());
  FilterConfig::parse_yaml_value(name, value)
}

#[cfg(test)]
mod test {
  use super::parse_config;
  use crate::filter::FilterConfig;

  fn assert_parse(inline: &str, full: &str) {
    let inline = parse_config(inline).unwrap();
    let full = FilterConfig::parse_yaml(full).unwrap();
    assert_eq!(inline, full);
  }

  #[test]
  fn test_parsing() {
    assert_parse("discard=foo", "discard: foo");
    // assert_parse("keep_only=bar", "keep_only: bar");
    // // numbers are parsed as expected
    // assert_parse("limit=1", "limit: 1");
    // assert_parse("limit=1h", "limit: '1h'");
    // assert_parse("simplify_html", "simplify_html: {}");
  }
}
