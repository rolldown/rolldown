use rolldown_utils::indexmap::FxIndexMap;
use rustc_hash::FxHashMap;
use std::{fmt::Debug, path::PathBuf};
use types::advanced_chunks_options::AdvancedChunksOptions;
use types::checks_options::ChecksOptions;
use types::comments::Comments;
use types::inject_import::InjectImport;
use types::jsx::Jsx;
use types::output_option::GlobalsOutputOption;
use types::target::ESTarget;
use types::watch_option::WatchOption;

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};
#[cfg(feature = "deserialize_bundler_options")]
use serde_json::Value;
use types::experimental_options::ExperimentalOptions;

use self::types::treeshake::TreeshakeOptions;
use self::types::{
  es_module_flag::EsModuleFlag, hash_characters::HashCharacters, input_item::InputItem,
  is_external::IsExternal, output_exports::OutputExports, output_format::OutputFormat,
  output_option::AddonOutputOption, platform::Platform, resolve_options::ResolveOptions,
  source_map_type::SourceMapType, sourcemap_path_transform::SourceMapPathTransform,
};
use crate::{ChunkFilenamesOutputOption, ModuleType, SourceMapIgnoreList};

pub mod types;

#[derive(Default, Debug, Clone)]
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
  pub file: Option<String>,
  pub format: Option<OutputFormat>,
  pub exports: Option<OutputExports>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_globals"),
    schemars(with = "Option<FxHashMap<String, String>>")
  )]
  pub globals: Option<GlobalsOutputOption>,
  pub sourcemap: Option<SourceMapType>,
  pub es_module: Option<EsModuleFlag>,
  pub drop_labels: Option<Vec<String>>,
  pub hash_characters: Option<HashCharacters>,
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
  pub sourcemap_debug_ids: Option<bool>,

  /// Key is the file extension. The extension should start with a `.`. E.g. `".txt"`.
  pub module_types: Option<FxHashMap<String, ModuleType>>,
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
    schemars(with = "Option<FxHashMap<String, String>>")
  )]
  pub define: Option<FxIndexMap<String, String>>,
  pub extend: Option<bool>,
  pub profiler_names: Option<bool>,
  pub keep_names: Option<bool>,
  pub inject: Option<Vec<InjectImport>>,
  pub external_live_bindings: Option<bool>,
  pub inline_dynamic_imports: Option<bool>,
  pub advanced_chunks: Option<AdvancedChunksOptions>,
  pub checks: Option<ChecksOptions>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(deserialize_with = "deserialize_jsx", default),
    schemars(with = "Option<FxHashMap<String, String>>")
  )]
  pub jsx: Option<Jsx>,
  pub watch: Option<WatchOption>,
  pub comments: Option<Comments>,
  pub target: Option<ESTarget>,
  pub polyfill_require: Option<bool>,
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
fn deserialize_globals<'de, D>(deserializer: D) -> Result<Option<GlobalsOutputOption>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<FxHashMap<String, String>>::deserialize(deserializer)?;
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
        annotations: Some(true),
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
      let annotations = obj.get("annotations").map_or_else(
        || Ok(Some(true)),
        |v| match v {
          Value::Bool(b) => Ok(Some(*b)),
          _ => Err(serde::de::Error::custom("annotations should be a `true` or `false`")),
        },
      )?;
      Ok(TreeshakeOptions::Option(types::treeshake::InnerOptions {
        module_side_effects,
        annotations,
      }))
    }
    _ => Err(serde::de::Error::custom("treeshake should be a boolean or an object")),
  }
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_jsx<'de, D>(deserializer: D) -> Result<Option<Jsx>, D::Error>
where
  D: Deserializer<'de>,
{
  use oxc::transformer::{JsxOptions, JsxRuntime};

  let value = Option::<Value>::deserialize(deserializer)?;
  match value {
    None => Ok(Some(Jsx::default())),
    Some(Value::Object(obj)) => {
      let mut default_jsx_option = JsxOptions::default();
      for (k, v) in obj {
        match k.as_str() {
          "runtime" => {
            let runtime = v
              .as_str()
              .ok_or_else(|| serde::de::Error::custom("jsx.pragma should be a string"))?;
            match runtime {
              "classic" => default_jsx_option.runtime = JsxRuntime::Classic,
              "automatic" => default_jsx_option.runtime = JsxRuntime::Automatic,
              _ => {
                return Err(serde::de::Error::custom(format!("unknown jsx runtime: {runtime}",)))
              }
            }
          }
          "importSource" => {
            let import_source = v
              .as_str()
              .ok_or_else(|| serde::de::Error::custom("jsx.importSource should be a string"))?;
            default_jsx_option.import_source = Some(import_source.to_string());
          }
          "development" => {
            let development = v
              .as_bool()
              .ok_or_else(|| serde::de::Error::custom("jsx.development should be a boolean"))?;
            default_jsx_option.development = development;
          }
          "pragma" => {
            let pragma = v
              .as_str()
              .ok_or_else(|| serde::de::Error::custom("jsx.pragma should be a string"))?;
            default_jsx_option.pragma = Some(pragma.to_string());
          }
          "pragmaFrag" => {
            let pragma_frag = v
              .as_str()
              .ok_or_else(|| serde::de::Error::custom("jsx.pragmaFrag should be a string"))?;
            default_jsx_option.pragma_frag = Some(pragma_frag.to_string());
          }
          _ => return Err(serde::de::Error::custom(format!("unknown jsx option: {k}",))),
        }
      }

      Ok(Some(Jsx::Enable(default_jsx_option)))
    }
    _ => Err(serde::de::Error::custom("jsx should be an object")),
  }
}
