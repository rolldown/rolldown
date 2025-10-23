#[cfg(feature = "deserialize_dev_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_dev_options")]
use serde::Deserialize;

#[derive(Debug, Default)]
#[cfg_attr(feature = "deserialize_dev_options", derive(Deserialize, JsonSchema))]
#[cfg_attr(
  feature = "deserialize_dev_options",
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct DevWatchOptions {
  /// If `true`, watcher will be disabled.
  pub disable_watcher: Option<bool>,
  /// If `true`, files are not written to disk.
  pub skip_write: Option<bool>,
  /// If `true`, use polling instead of native file system events for watching
  pub use_polling: Option<bool>,
  /// Poll interval in milliseconds (only used when use_polling is true)
  pub poll_interval: Option<u64>,
  /// If `true`, use debounced watcher. If `false`, use non-debounced watcher
  pub use_debounce: Option<bool>,
  /// Debounce duration in milliseconds (only used when use_debounce is true)
  pub debounce_duration: Option<u64>,
  /// Whether to compare file contents for poll-based watchers (only used when use_polling is true)
  pub compare_contents_for_polling: Option<bool>,
  /// Tick rate in milliseconds for debounced watchers (only used when use_debounce is true)
  pub debounce_tick_rate: Option<u64>,
}
