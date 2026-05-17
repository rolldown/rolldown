#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

use super::attach_debug_info::AttachDebugInfo;
use super::chunk_import_map::ChunkImportMap;
use super::chunk_modules_order::ChunkModulesOrderBy;
use super::dev_mode_options::DevModeOptions;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields, default)
)]
pub struct ChunkOptimizationOptions {
  pub merge_common_chunks: bool,
  pub avoid_redundant_chunk_loads: bool,
}

impl Default for ChunkOptimizationOptions {
  fn default() -> Self {
    Self { merge_common_chunks: true, avoid_redundant_chunk_loads: true }
  }
}

impl ChunkOptimizationOptions {
  pub fn is_merge_common_chunks_enabled(&self) -> bool {
    self.merge_common_chunks
  }

  pub fn is_avoid_redundant_chunk_loads_enabled(&self) -> bool {
    self.avoid_redundant_chunk_loads
  }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(untagged)
)]
pub enum ChunkOptimizationOption {
  Bool(bool),
  Options(ChunkOptimizationOptions),
}

impl ChunkOptimizationOption {
  pub fn is_merge_common_chunks_enabled(&self) -> bool {
    match self {
      Self::Bool(enabled) => *enabled,
      Self::Options(options) => options.is_merge_common_chunks_enabled(),
    }
  }

  pub fn is_avoid_redundant_chunk_loads_enabled(&self) -> bool {
    match self {
      Self::Bool(enabled) => *enabled,
      Self::Options(options) => options.is_avoid_redundant_chunk_loads_enabled(),
    }
  }
}

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
  pub chunk_optimization: Option<ChunkOptimizationOption>,
  pub lazy_barrel: Option<bool>,
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

  pub fn is_merge_common_chunks_enabled(&self) -> bool {
    self
      .chunk_optimization
      .as_ref()
      .is_none_or(ChunkOptimizationOption::is_merge_common_chunks_enabled)
  }

  pub fn is_avoid_redundant_chunk_loads_enabled(&self) -> bool {
    self
      .chunk_optimization
      .as_ref()
      .is_none_or(ChunkOptimizationOption::is_avoid_redundant_chunk_loads_enabled)
  }

  pub fn is_lazy_barrel_enabled(&self) -> bool {
    self.lazy_barrel.unwrap_or(false)
  }
}
