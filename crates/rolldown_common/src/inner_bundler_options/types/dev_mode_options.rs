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
pub struct DevModeOptions {
  /// IP addresses that `DevRuntime` will connect to using WebSocket.
  pub host: Option<String>,
  /// Port that `DevRuntime` will connect to using WebSocket.
  pub port: Option<u16>,
  /// Custom dev mode runtime implementation.
  pub implement: Option<String>,
  /// Do not inject the common dev runtime before the custom implementation.
  ///
  /// Deprecated: common runtime injection will be disabled by default in the future.
  pub skip_common_runtime_injection: Option<bool>,
  /// Enable lazy compilation for dynamic imports.
  pub lazy: Option<bool>,
}
