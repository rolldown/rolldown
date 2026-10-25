use rolldown_utils::pattern_filter::{self, StringOrRegex};
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

/// Limits the modules `inject` applies to, like the `include`/`exclude` options of `@rollup/plugin-inject`.
#[derive(Debug, Default, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct InjectFilter {
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

impl InjectFilter {
  pub fn is_empty(&self) -> bool {
    self.include.is_none() && self.exclude.is_none()
  }

  pub fn matches(&self, id: &str, cwd: &str) -> bool {
    pattern_filter::filter(self.exclude.as_deref(), self.include.as_deref(), id, cwd).inner()
  }
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
