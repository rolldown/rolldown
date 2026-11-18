use std::sync::Arc;

use derive_more::Debug;

use rolldown_utils::pattern_filter::StringOrRegex;
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

#[derive(Debug, Default, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct WatchOption {
  pub skip_write: bool,
  pub build_delay: Option<u32>,
  pub watcher: Option<WatcherFileWatcherOptions>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_string_or_regex"),
    schemars(with = "Option<Vec<String>>")
  )]
  pub include: Option<Vec<StringOrRegex>>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_string_or_regex"),
    schemars(with = "Option<Vec<String>>")
  )]
  pub exclude: Option<Vec<StringOrRegex>>,
  #[debug("Function")]
  #[cfg_attr(feature = "deserialize_bundler_options", serde(skip_serializing, skip_deserializing))]
  pub on_invalidate: Option<OnInvalidate>,
}

/// Filesystem watcher settings shared by build watch and the dev engine.
/// See internal-docs/watch-mode/implementation.md#backend-selection.
#[derive(Debug, Default, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct WatcherFileWatcherOptions {
  /// Use polling instead of native filesystem events.
  pub use_polling: Option<bool>,
  /// Poll interval in milliseconds, when polling is enabled.
  pub poll_interval: Option<u64>,
  /// Compare file contents to detect changes when polling is enabled.
  pub compare_contents_for_polling: Option<bool>,
  /// Coalesce filesystem events before delivering them to the build coordinator.
  pub use_debounce: Option<bool>,
  /// Debounce delay in milliseconds, when debouncing is enabled.
  pub debounce_delay: Option<u64>,
  /// Debouncer tick rate in milliseconds; defaults to one quarter of the debounce delay.
  pub debounce_tick_rate: Option<u64>,
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_string_or_regex<'de, D>(
  deserializer: D,
) -> Result<Option<Vec<StringOrRegex>>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<Vec<String>>::deserialize(deserializer)?;
  Ok(deserialized.map(|v| v.into_iter().map(StringOrRegex::String).collect::<Vec<_>>()))
}

// TODO should it be just placed here?
type OnInvalidateFn = dyn Fn(&str) + Send + Sync;

#[derive(Clone, Debug)]
#[debug("OnInvalidateFn::Fn(...)")]
pub struct OnInvalidate(Arc<OnInvalidateFn>);

impl OnInvalidate {
  pub fn new(f: Arc<OnInvalidateFn>) -> Self {
    Self(f)
  }

  pub fn call(&self, path: &str) {
    (self.0)(path);
  }
}
