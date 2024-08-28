use rolldown_utils::indexmap::FxIndexMap;
use std::{collections::HashMap, fmt::Debug, path::PathBuf};
use types::inject_import::InjectImport;

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};
#[cfg(feature = "deserialize_bundler_options")]
use serde_json::Value;
use types::experimental_options::ExperimentalOptions;

use self::types::treeshake::TreeshakeOptions;
use self::types::{
  es_module_flag::EsModuleFlag, input_item::InputItem, is_external::IsExternal,
  output_exports::OutputExports, output_format::OutputFormat, output_option::AddonOutputOption,
  platform::Platform, resolve_options::ResolveOptions, source_map_type::SourceMapType,
  sourcemap_path_transform::SourceMapPathTransform,
};
use crate::{ChunkFilenamesOutputOption, ModuleType, SourceMapIgnoreList};

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
  pub name: Option<String>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_chunk_filenames"),
    schemars(with = "Option<String>")
  )]
  pub entry_filenames: Option<ChunkFilenamesOutputOption>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_chunk_filenames"),
    schemars(with = "Option<String>")
  )]
  pub chunk_filenames: Option<ChunkFilenamesOutputOption>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_chunk_filenames"),
    schemars(with = "Option<String>")
  )]
  pub css_entry_filenames: Option<ChunkFilenamesOutputOption>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_chunk_filenames"),
    schemars(with = "Option<String>")
  )]
  pub css_chunk_filenames: Option<ChunkFilenamesOutputOption>,
  pub asset_filenames: Option<String>,
  pub dir: Option<String>,
  pub format: Option<OutputFormat>,
  pub exports: Option<OutputExports>,
  pub globals: Option<HashMap<String, String>>,
  pub sourcemap: Option<SourceMapType>,
  pub es_module: Option<EsModuleFlag>,
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
    serde(default, deserialize_with = "deserialize_addon"),
    schemars(with = "Option<String>")
  )]
  pub intro: Option<AddonOutputOption>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_addon"),
    schemars(with = "Option<String>")
  )]
  pub outro: Option<AddonOutputOption>,
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
    serde(deserialize_with = "deserialize_treeshake", default)
  )]
  pub treeshake: TreeshakeOptions,
  pub experimental: Option<ExperimentalOptions>,
  pub minify: Option<bool>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    schemars(with = "Option<HashMap<String, String>>")
  )]
  pub define: Option<FxIndexMap<String, String>>,
  pub extend: Option<bool>,
  pub inject: Option<Vec<InjectImport>>,
  pub external_live_bindings: Option<bool>,
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
fn deserialize_chunk_filenames<'de, D>(
  deserializer: D,
) -> Result<Option<ChunkFilenamesOutputOption>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<String>::deserialize(deserializer)?;
  Ok(deserialized.map(From::from))
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_treeshake<'de, D>(deserializer: D) -> Result<TreeshakeOptions, D::Error>
where
  D: Deserializer<'de>,
{
  let value = Option::<Value>::deserialize(deserializer)?;
  match value {
    Some(Value::Bool(false)) => Ok(TreeshakeOptions::Boolean(false)),
    None | Some(Value::Bool(true)) => {
      Ok(TreeshakeOptions::Option(types::treeshake::InnerOptions {
        module_side_effects: types::treeshake::ModuleSideEffects::Boolean(true),
      }))
    }
    Some(Value::Object(obj)) => {
      let module_side_effects = obj.get("moduleSideEffects").map_or_else(
        || Ok(types::treeshake::ModuleSideEffects::Boolean(true)),
        |v| match v {
          Value::Bool(b) => Ok(types::treeshake::ModuleSideEffects::Boolean(*b)),
          _ => Err(serde::de::Error::custom("moduleSideEffects should be a `true` or `false`")),
        },
      )?;
      Ok(TreeshakeOptions::Option(types::treeshake::InnerOptions { module_side_effects }))
    }
    _ => Err(serde::de::Error::custom("treeshake should be a boolean or an object")),
  }
}
