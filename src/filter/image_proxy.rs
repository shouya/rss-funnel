use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct ImageProxyConfig {
  /// Only rewrite images whose url matches one of the given
  /// domains. Globbing is supported: "*.example.com" matches
  /// "foo.example.com" but not "example.com".
  domains: Option<Vec<String>>,
  /// Only rewrite urls of <img> tags matching the following CSS
  /// selector.
  selector: Option<String>,
  #[serde(flatten)]
  settings: Settings,
}

#[derive(Deserialize, Debug)]
enum Settings {
  External(ExternalConfig),
  #[serde(untagged)]
  // TODO: check if RSS_FUNNEL_IMAGE_PROXY is set, or raise a warning.
  Internal(crate::server::image_proxy::Config),
}

#[derive(Deserialize, Debug)]
struct ExternalConfig {
  /// The base URL to append the images to.
  base: String,
  /// Whether to urlencode the images urls before appending them to
  /// the base. If base ends with a "=", this option defaults to true,
  /// otherwise it defaults to false.
  urlencode: Option<bool>,
}

struct InternalConfig {}

pub struct ImageProxy {
  config: ImageProxyConfig,
}
