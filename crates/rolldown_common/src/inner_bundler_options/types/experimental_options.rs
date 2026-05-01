#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

use super::attach_debug_info::AttachDebugInfo;
use super::chunk_import_map::ChunkImportMap;
use super::chunk_modules_order::ChunkModulesOrderBy;
use super::dev_mode_options::DevModeOptions;

#[derive(Debug, Default, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct ExperimentalOptions {
  pub vite_mode: Option<bool>,
  pub resolve_new_url_to_asset: Option<bool>,
  pub incremental_build: Option<bool>,
  pub dev_mode: Option<DevModeOptions>,
  pub attach_debug_info: Option<AttachDebugInfo>,
  pub chunk_import_map: Option<ChunkImportMap>,
  pub chunk_modules_order: Option<ChunkModulesOrderBy>,
  pub on_demand_wrapping: Option<bool>,
  pub native_magic_string: Option<bool>,
  pub chunk_optimization: Option<bool>,
  pub lazy_barrel: Option<bool>,
  pub already_loaded_atom_propagation: Option<bool>,
}

impl ExperimentalOptions {
  pub fn is_on_demand_wrapping_enabled(&self) -> bool {
    self.on_demand_wrapping.unwrap_or(false)
  }

  pub fn is_resolve_new_url_to_asset_enabled(&self) -> bool {
    self.resolve_new_url_to_asset.unwrap_or(false)
  }

  #[inline]
  pub fn is_incremental_build_enabled(&self) -> bool {
    self.incremental_build.unwrap_or(false)
  }

  pub fn is_attach_debug_info_enabled(&self) -> bool {
    self.attach_debug_info.is_some_and(|info| info.is_enabled())
  }

  pub fn is_attach_debug_info_full(&self) -> bool {
    self.attach_debug_info.is_some_and(|info| info.is_full())
  }

  pub fn is_native_magic_string_enabled(&self) -> bool {
    self.native_magic_string.unwrap_or(false)
  }

  pub fn is_chunk_optimization_enabled(&self) -> bool {
    self.chunk_optimization.unwrap_or(true)
  }

  pub fn is_lazy_barrel_enabled(&self) -> bool {
    self.lazy_barrel.unwrap_or(false)
  }

  /// Pre-grouping pass that strips redundant dynamic-entry bits from
  /// modules guaranteed already-in-memory at the dynamic entry's load
  /// time. Mirrors Rollup's `getAlreadyLoadedAtomsByEntry`. Enabled by
  /// default. Strips are gated against `preserveEntrySignatures: 'strict'`
  /// targets and rolled back when they would create a static-import cycle
  /// in the projected post-strip chunk graph (manual chunks are visible
  /// to the cycle projection because the pass runs after
  /// `apply_manual_code_splitting`). Known remaining gaps: awaited vs
  /// non-awaited dynamic imports are not distinguished (TLA hang on
  /// specific topologies), and runtime-helper consumer placement may
  /// differ from the pre-pass topology in some graphs.
  pub fn is_already_loaded_atom_propagation_enabled(&self) -> bool {
    self.already_loaded_atom_propagation.unwrap_or(true)
  }
}
