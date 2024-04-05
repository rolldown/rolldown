use std::{fmt::Debug, path::PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer};

use self::types::{
  external::External, input_item::InputItem, output_format::OutputFormat,
  output_option::AddonOutputOption, platform::Platform, resolve_options::ResolveOptions,
  source_map_type::SourceMapType,
};

pub mod types;

#[derive(Default, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundlerOptions {
  // --- options for output
  pub input: Option<Vec<InputItem>>,
  pub cwd: Option<PathBuf>,
  #[serde(default, deserialize_with = "deserialize_external")]
  #[schemars(with = "Option<Vec<String>>")]
  pub external: Option<External>,
  pub treeshake: Option<bool>,
  pub platform: Option<Platform>,
  // --- options for output
  pub entry_file_names: Option<String>,
  pub chunk_file_names: Option<String>,
  pub dir: Option<String>,
  pub format: Option<OutputFormat>,
  pub sourcemap: Option<SourceMapType>,
  #[serde(default, deserialize_with = "deserialize_addon")]
  #[schemars(with = "Option<String>")]
  pub banner: Option<AddonOutputOption>,
  #[serde(default, deserialize_with = "deserialize_addon")]
  #[schemars(with = "Option<String>")]
  pub footer: Option<AddonOutputOption>,
  // --- options for resolve
  pub resolve: Option<ResolveOptions>,
}

fn deserialize_external<'de, D>(deserializer: D) -> Result<Option<External>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<Vec<String>>::deserialize(deserializer)?;
  Ok(deserialized.map(External::ArrayString))
}

fn deserialize_addon<'de, D>(deserializer: D) -> Result<Option<AddonOutputOption>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<String>::deserialize(deserializer)?;
  Ok(deserialized.map(|s| AddonOutputOption::String(Some(s))))
}
