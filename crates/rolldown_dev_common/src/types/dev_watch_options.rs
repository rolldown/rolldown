use rolldown_common::WatcherFileWatcherOptions;
use rolldown_utils::pattern_filter::StringOrRegex;
#[cfg(feature = "deserialize_dev_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_dev_options")]
use serde::{Deserialize, Deserializer};

#[derive(Debug, Default)]
#[cfg_attr(feature = "deserialize_dev_options", derive(Deserialize, JsonSchema))]
#[cfg_attr(
  feature = "deserialize_dev_options",
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct DevWatchOptions {
  /// Whether to watch for filesystem changes. Defaults to `true`.
  /// Initial and explicit builds are available when watching is disabled.
  pub enabled: Option<bool>,
  /// If `true`, files are not written to disk.
  #[cfg_attr(feature = "deserialize_dev_options", serde(default))]
  pub skip_write: bool,
  /// Filesystem watcher settings. Debounced events are enabled by default in dev mode.
  pub watcher: Option<WatcherFileWatcherOptions>,
  /// Filter to limit which discovered files are registered with the file watcher.
  /// Strings are treated as glob patterns.
  #[cfg_attr(
    feature = "deserialize_dev_options",
    serde(default, deserialize_with = "deserialize_string_or_regex"),
    schemars(with = "Option<Vec<String>>")
  )]
  pub include: Option<Vec<StringOrRegex>>,
  /// Filter to prevent discovered files from being registered with the file watcher.
  /// Strings are treated as glob patterns.
  #[cfg_attr(
    feature = "deserialize_dev_options",
    serde(default, deserialize_with = "deserialize_string_or_regex"),
    schemars(with = "Option<Vec<String>>")
  )]
  pub exclude: Option<Vec<StringOrRegex>>,
}

#[cfg(feature = "deserialize_dev_options")]
fn deserialize_string_or_regex<'de, D>(
  deserializer: D,
) -> Result<Option<Vec<StringOrRegex>>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<Vec<String>>::deserialize(deserializer)?;
  Ok(deserialized.map(|v| v.into_iter().map(StringOrRegex::String).collect::<Vec<_>>()))
}
