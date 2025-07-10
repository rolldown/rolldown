use std::{sync::Arc, time::Duration};

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

#[derive(Debug, Default, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct NotifyOption {
  pub poll_interval: Option<Duration>,
  pub compare_contents: bool,
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
