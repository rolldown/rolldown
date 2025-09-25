#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Default, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct HmrOptions {
  /// IP addresses that `DevRuntime` will connect to using WebSocket.
  pub host: Option<String>,
  /// Port that `DevRuntime` will connect to using WebSocket.
  pub port: Option<u16>,
  /// Custom hmr runtime implementation.
  pub implement: Option<String>,
}
