use std::time::Duration;

use rolldown_utils::pattern_filter::StringOrRegex;
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

#[derive(Debug, Default)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct WatchOption {
  pub skip_write: bool,
  pub notify: Option<NotifyOption>,
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

#[derive(Debug, Default)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct NotifyOption {
  pub poll_interval: Option<Duration>,
  pub compare_contents: bool,
}
