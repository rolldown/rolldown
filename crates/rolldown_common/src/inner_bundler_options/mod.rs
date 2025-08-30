use rolldown_utils::indexmap::FxIndexMap;
use rustc_hash::FxHashMap;
use std::{fmt::Debug, path::PathBuf};
use types::advanced_chunks_options::AdvancedChunksOptions;
use types::debug_options::DebugOptions;
use types::inject_import::InjectImport;
use types::invalidate_js_side_cache::InvalidateJsSideCache;
use types::legal_comments::LegalComments;
use types::log_level::LogLevel;
use types::make_absolute_externals_relative::MakeAbsoluteExternalsRelative;
use types::mark_module_loaded::MarkModuleLoaded;
use types::minify_options::RawMinifyOptions;
use types::on_log::OnLog;
use types::optimization::OptimizationOption;
use types::output_option::{
  AssetFilenamesOutputOption, GlobalsOutputOption, PreserveEntrySignatures,
};
use types::sanitize_filename::SanitizeFilename;
use types::watch_option::WatchOption;

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};
#[cfg(feature = "deserialize_bundler_options")]
use serde_json::Value;
use types::experimental_options::ExperimentalOptions;
#[cfg(feature = "deserialize_bundler_options")]
use types::minify_options::SimpleMinifyOptions;

use self::types::treeshake::TreeshakeOptions;
use self::types::{
  defer_sync_scan_data_option::DeferSyncScanDataOption, es_module_flag::EsModuleFlag,
  hash_characters::HashCharacters, input_item::InputItem, is_external::IsExternal,
  output_exports::OutputExports, output_format::OutputFormat, output_option::AddonOutputOption,
  platform::Platform, resolve_options::ResolveOptions, source_map_type::SourceMapType,
  sourcemap_path_transform::SourceMapPathTransform,
};

use crate::{
  BundlerTransformOptions, ChecksOptions, ChunkFilenamesOutputOption, ModuleType,
  SourceMapIgnoreList,
};

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
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_asset_filenames"),
    schemars(with = "Option<String>")
  )]
  pub asset_filenames: Option<AssetFilenamesOutputOption>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_sanitize_filename"),
    schemars(with = "Option<bool>")
  )]
  pub sanitize_filename: Option<SanitizeFilename>,
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
  pub sourcemap_base_url: Option<String>,
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
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(deserialize_with = "deserialize_minify", default),
    schemars(with = "SimpleMinifyOptions")
  )]
  pub minify: Option<RawMinifyOptions>,
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
    serde(deserialize_with = "deserialize_transform_options", default),
    schemars(with = "Option<FxHashMap<String, Value>>")
  )]
  pub transform: Option<BundlerTransformOptions>,
  pub watch: Option<WatchOption>,
  pub legal_comments: Option<LegalComments>,
  pub polyfill_require: Option<bool>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, skip_deserializing),
    schemars(skip)
  )]
  pub defer_sync_scan_data: Option<DeferSyncScanDataOption>,
  pub make_absolute_externals_relative: Option<MakeAbsoluteExternalsRelative>,
  pub debug: Option<DebugOptions>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, skip_deserializing),
    schemars(skip)
  )]
  pub invalidate_js_side_cache: Option<InvalidateJsSideCache>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, skip_deserializing),
    schemars(skip)
  )]
  pub mark_module_loaded: Option<MarkModuleLoaded>,
  pub log_level: Option<LogLevel>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, skip_deserializing),
    schemars(skip)
  )]
  pub on_log: Option<OnLog>,
  pub preserve_modules: Option<bool>,
  pub virtual_dirname: Option<String>,
  pub preserve_modules_root: Option<String>,
  pub preserve_entry_signatures: Option<PreserveEntrySignatures>,
  pub optimization: Option<OptimizationOption>,
  pub top_level_var: Option<bool>,
  pub minify_internal_exports: Option<bool>,
  pub context: Option<String>,
  pub tsconfig: Option<String>,
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
fn deserialize_asset_filenames<'de, D>(
  deserializer: D,
) -> Result<Option<AssetFilenamesOutputOption>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<String>::deserialize(deserializer)?;
  Ok(deserialized.map(From::from))
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_sanitize_filename<'de, D>(
  deserializer: D,
) -> Result<Option<SanitizeFilename>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<bool>::deserialize(deserializer)?;
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
fn deserialize_minify<'de, D>(deserializer: D) -> Result<Option<RawMinifyOptions>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<SimpleMinifyOptions>::deserialize(deserializer)?;
  match deserialized {
    Some(SimpleMinifyOptions::Boolean(false)) => Ok(Some(RawMinifyOptions::Bool(false))),
    Some(SimpleMinifyOptions::Boolean(true)) => Ok(Some(RawMinifyOptions::Bool(true))),
    Some(SimpleMinifyOptions::String(value)) if value == "dceOnly" => {
      Ok(Some(RawMinifyOptions::DeadCodeEliminationOnly))
    }
    None => Ok(None),
    _ => unreachable!("Unexpected value for minify {:?}", deserialized),
  }
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_treeshake<'de, D>(deserializer: D) -> Result<TreeshakeOptions, D::Error>
where
  D: Deserializer<'de>,
{
  use rustc_hash::FxHashSet;

  let value = Option::<Value>::deserialize(deserializer)?;
  match value {
    Some(Value::Bool(false)) => Ok(TreeshakeOptions::Boolean(false)),
    None | Some(Value::Bool(true)) => {
      Ok(TreeshakeOptions::Option(types::treeshake::InnerOptions {
        module_side_effects: types::treeshake::ModuleSideEffects::Boolean(true),
        annotations: Some(true),
        manual_pure_functions: None,
        unknown_global_side_effects: None,
        commonjs: Some(true),
        property_read_side_effects: None,
        property_write_side_effects: None,
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
      let commonjs = obj.get("commonjs").map_or_else(
        || Ok(Some(true)),
        |v| match v {
          Value::Bool(b) => Ok(Some(*b)),
          _ => Err(serde::de::Error::custom("commonjs should be a `true` or `false`")),
        },
      )?;
      let unknown_global_side_effects = obj.get("unknown_global_side_effects").map_or_else(
        || Ok(Some(true)),
        |v| match v {
          Value::Bool(b) => Ok(Some(*b)),
          _ => Err(serde::de::Error::custom(
            "unknown_global_side_effects should be a `true` or `false`",
          )),
        },
      )?;
      let manual_pure_functions = obj.get("manualPureFunctions").map_or_else(
        || Ok(FxHashSet::default()),
        |v| match v {
          Value::Array(v) => Ok(
            v.iter()
              .map(|item| {
                item.as_str().expect("manualPureFunctions should be a `Vec<String>`").to_string()
              })
              .collect::<FxHashSet<_>>(),
          ),
          _ => Err(serde::de::Error::custom("manualPureFunctions should be a `Vec<String>`")),
        },
      )?;
      // Use string to make deserialization logic easier
      let property_read_side_effects = obj.get("propertyReadSideEffects").map_or_else(
        || Ok(None),
        |v| match v {
          Value::String(s) if s == "always" => {
            Ok(Some(types::treeshake::PropertyReadSideEffects::Always))
          }
          Value::String(s) if s == "false" => {
            Ok(Some(types::treeshake::PropertyReadSideEffects::False))
          }
          _ => {
            Err(serde::de::Error::custom("propertyReadSideEffects should be 'false' or 'always'"))
          }
        },
      )?;
      let property_write_side_effects = obj.get("propertyWriteSideEffects").map_or_else(
        || Ok(None),
        |v| match v {
          Value::String(s) if s == "always" => {
            Ok(Some(types::treeshake::PropertyWriteSideEffects::Always))
          }
          Value::String(s) if s == "false" => {
            Ok(Some(types::treeshake::PropertyWriteSideEffects::False))
          }
          _ => {
            Err(serde::de::Error::custom("propertyWriteSideEffects should be 'false' or 'always'"))
          }
        },
      )?;
      Ok(TreeshakeOptions::Option(types::treeshake::InnerOptions {
        module_side_effects,
        annotations,
        manual_pure_functions: Some(manual_pure_functions),
        unknown_global_side_effects,
        commonjs,
        property_read_side_effects,
        property_write_side_effects,
      }))
    }
    _ => Err(serde::de::Error::custom("treeshake should be a boolean or an object")),
  }
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_transform_options<'de, D>(
  deserializer: D,
) -> Result<Option<BundlerTransformOptions>, D::Error>
where
  D: Deserializer<'de>,
{
  use itertools::Either;
  use serde_json::Value;

  use crate::bundler_options::JsxOptions;

  let value = Option::<Value>::deserialize(deserializer)?;
  match value {
    Some(Value::Object(obj)) => {
      let mut transform_options = BundlerTransformOptions::default();
      for (k, v) in obj {
        match k.as_str() {
          "target" => {
            let target = v
              .as_str()
              .ok_or_else(|| serde::de::Error::custom("transform.target should be a string"))?;
            transform_options.target = Some(Either::Left(target.to_string()));
          }
          "jsx" => {
            transform_options.jsx = match v {
              Value::String(str) if str == "preserve" => Some(Either::Left(str)),
              Value::Object(obj) => {
                let mut default_jsx_option = JsxOptions::default();
                for (k, v) in obj {
                  match k.as_str() {
                    "runtime" => {
                      let runtime = v.as_str().ok_or_else(|| {
                        serde::de::Error::custom("jsx.runtime should be a string")
                      })?;
                      match runtime {
                        "classic" | "automatic" => {
                          default_jsx_option.runtime = Some(runtime.to_owned());
                        }
                        _ => {
                          return Err(serde::de::Error::custom(format!(
                            "unknown jsx runtime: {runtime}",
                          )));
                        }
                      }
                    }
                    "importSource" => {
                      let import_source = v.as_str().ok_or_else(|| {
                        serde::de::Error::custom("jsx.importSource should be a string")
                      })?;
                      default_jsx_option.import_source = Some(import_source.to_owned());
                    }
                    "development" => {
                      let development = v.as_bool().ok_or_else(|| {
                        serde::de::Error::custom("jsx.development should be a boolean")
                      })?;
                      default_jsx_option.development = Some(development);
                    }
                    "pragma" => {
                      let pragma = v
                        .as_str()
                        .ok_or_else(|| serde::de::Error::custom("jsx.pragma should be a string"))?;
                      default_jsx_option.pragma = Some(pragma.to_owned());
                    }
                    "pragmaFrag" => {
                      let pragma_frag = v.as_str().ok_or_else(|| {
                        serde::de::Error::custom("jsx.pragmaFrag should be a string")
                      })?;
                      default_jsx_option.pragma_frag = Some(pragma_frag.to_owned());
                    }
                    _ => return Err(serde::de::Error::custom(format!("unknown jsx option: {k}",))),
                  }
                }
                Some(Either::Right(default_jsx_option))
              }
              _ => {
                return Err(serde::de::Error::custom(
                  "jsx should be either an object or `preserve`",
                ));
              }
            };
          }
          _ => return Err(serde::de::Error::custom(format!("unknown transform option: {k}",))),
        }
      }
      Ok(Some(transform_options))
    }
    _ => Err(serde::de::Error::custom("transform options should be an object")),
  }
}
