use std::{collections::HashMap, fmt::Debug, path::PathBuf};

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

use crate::{ModuleType, SourceMapIgnoreList};

use self::types::treeshake::TreeshakeOptions;
use self::types::{
  input_item::InputItem, is_external::IsExternal, output_format::OutputFormat,
  output_option::AddonOutputOption, platform::Platform, resolve_options::ResolveOptions,
  source_map_type::SourceMapType, sourcemap_path_transform::SourceMapPathTransform,
};

pub mod types;

#[derive(Default, Debug)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct BundlerOptions {
  // --- options for input
  pub input: Option<Vec<InputItem>>,
  pub cwd: Option<PathBuf>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_external"),
    schemars(with = "Option<Vec<String>>")
  )]
  pub external: Option<IsExternal>,
  pub platform: Option<Platform>,
  pub shim_missing_exports: Option<bool>,
  // --- options for output
  pub entry_filenames: Option<String>,
  pub chunk_filenames: Option<String>,
  pub asset_filenames: Option<String>,
  pub dir: Option<String>,
  pub format: Option<OutputFormat>,
  pub sourcemap: Option<SourceMapType>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_addon"),
    schemars(with = "Option<String>")
  )]
  pub banner: Option<AddonOutputOption>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_addon"),
    schemars(with = "Option<String>")
  )]
  pub footer: Option<AddonOutputOption>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, skip_deserializing),
    schemars(skip)
  )]
  pub sourcemap_ignore_list: Option<SourceMapIgnoreList>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, skip_deserializing),
    schemars(skip)
  )]
  pub sourcemap_path_transform: Option<SourceMapPathTransform>,

  /// Key is the file extension. The extension should start with a `.`. E.g. `".txt"`.
  pub module_types: Option<HashMap<String, ModuleType>>,
  // --- options for resolve
  pub resolve: Option<ResolveOptions>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(deserialize_with = "deserialize_treeshake", default),
    schemars(with = "Option<bool>")
  )]
  pub treeshake: TreeshakeOptions,
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_external<'de, D>(deserializer: D) -> Result<Option<IsExternal>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<Vec<String>>::deserialize(deserializer)?;
  Ok(deserialized.map(IsExternal::from_vec))
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_addon<'de, D>(deserializer: D) -> Result<Option<AddonOutputOption>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<String>::deserialize(deserializer)?;
  Ok(deserialized.map(|s| AddonOutputOption::String(Some(s))))
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_treeshake<'de, D>(deserializer: D) -> Result<TreeshakeOptions, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<bool>::deserialize(deserializer)?;
  match deserialized {
    Some(false) => Ok(TreeshakeOptions::False),
    Some(true) | None => Ok(TreeshakeOptions::Option(types::treeshake::InnerOptions {
      module_side_effects: types::treeshake::ModuleSideEffects::Boolean(true),
    })),
  }
}
