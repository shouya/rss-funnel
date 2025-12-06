use std::str::FromStr;

use tracing::warn;

use crate::{
  error::Result,
  feed::Feed,
  filter::{FilterConfig, FilterContext},
  filter_pipeline::{FilterPipeline, FilterPipelineConfig},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnTheFlyFilterQuery {
  query: Vec<String>,
}

impl OnTheFlyFilterQuery {
  pub fn from_uri_query(uri_query: &str) -> Self {
    let mut query = Vec::new();
    for param in uri_query.split('&') {
      let filter_name;
      if let Some((key, _)) = param.split_once('=') {
        filter_name = key;
      } else {
        filter_name = param;
      }

      if FilterConfig::is_valid_key(filter_name) {
        query.push(param.to_string());
      }
    }

    Self { query }
  }
}

struct FilterPipelineCache {
  query: OnTheFlyFilterQuery,
  pipeline: FilterPipeline,
}

#[derive(Default)]
pub struct OnTheFlyFilter {
  cache: Option<FilterPipelineCache>,
}

impl OnTheFlyFilter {
  async fn update(
    &mut self,
    query: OnTheFlyFilterQuery,
  ) -> Result<&FilterPipeline> {
    if self.cache.as_ref().is_some_and(|c| c.query == query) {
      return Ok(&self.cache.as_ref().unwrap().pipeline);
    }

    let pipeline_config = parse_pipeline_config(&query)?;
    let pipeline = FilterPipeline::from_config(pipeline_config).await?;

    self.cache = Some(FilterPipelineCache { query, pipeline });

    Ok(&self.cache.as_ref().unwrap().pipeline)
  }

  pub async fn run(
    &mut self,
    query: OnTheFlyFilterQuery,
    context: &mut FilterContext,
    feed: Feed,
  ) -> Result<Feed> {
    let pipeline = self.update(query).await?;
    pipeline.run(context, feed).await
  }
}

fn parse_pipeline_config(
  query: &OnTheFlyFilterQuery,
) -> Result<FilterPipelineConfig> {
  let configs = query
    .query
    .iter()
    .map(|s| parse_single(s))
    .collect::<Result<Vec<_>, _>>()?;
  Ok(FilterPipelineConfig::from(configs))
}

fn parse_single(param: &str) -> Result<FilterConfig> {
  use serde_yaml::{Mapping, Number, Value};
  if !param.contains('=') || param.ends_with('=') {
    let param = param.strip_suffix('=').unwrap_or(param);
    let value = Mapping::new().into();
    return FilterConfig::parse_yaml_value(param, value);
  }

  let Some((name, value)) = param.split_once('=') else {
    let message = format!("invalid on-the-fly param: {param}");
    warn!("{}", message);
    anyhow::bail!("{message}");
  };

  let Ok(value) = urlencoding::decode(value) else {
    let message =
      format!("invalid url decoding from on-the-fly param: {param}");
    warn!("{}", message);
    anyhow::bail!("{message}");
  };

  // try parse value as number if possible
  if let Ok(num) = Number::from_str(&value) {
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
  use super::parse_single;
  use crate::filter::FilterConfig;

  fn assert_parse(otf: &str, full: &str) {
    let otf = parse_single(otf).unwrap();
    let full = FilterConfig::parse_yaml(full).unwrap();
    assert_eq!(otf, full);
  }

  #[test]
  fn test_parsing() {
    assert_parse("discard=foo", "discard: foo");
    assert_parse("keep_only=bar", "keep_only: bar");
    // numbers are parsed as expected
    assert_parse("limit=1", "limit: 1");
    assert_parse("limit=1h", "limit: '1h'");
    assert_parse("discard=a%20b", "discard: 'a b'");
    // empty value are supported
    assert_parse("simplify_html", "simplify_html: {}");
    assert_parse("simplify_html=", "simplify_html: {}");
  }
}
