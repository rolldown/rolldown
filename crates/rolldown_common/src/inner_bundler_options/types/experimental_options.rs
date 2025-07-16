#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

use crate::ROLLDOWN_IGNORE;

use super::attach_debug_info::AttachDebugInfo;
use super::chunk_modules_order::ChunkModulesOrderBy;
use super::hmr_options::HmrOptions;

#[derive(Debug, Default, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct ExperimentalOptions {
  pub strict_execution_order: Option<bool>,
  pub disable_live_bindings: Option<bool>,
  pub vite_mode: Option<bool>,
  pub resolve_new_url_to_asset: Option<bool>,
  pub incremental_build: Option<bool>,
  pub hmr: Option<HmrOptions>,
  pub attach_debug_info: Option<AttachDebugInfo>,
  pub chunk_modules_order: Option<ChunkModulesOrderBy>,
  pub chunk_import_map: Option<bool>,
  pub on_demand_wrapping: Option<bool>,
}

impl ExperimentalOptions {
  pub fn is_strict_execution_order_enabled(&self) -> bool {
    self.strict_execution_order.unwrap_or(false)
  }

  pub fn is_disable_live_bindings_enabled(&self) -> bool {
    self.disable_live_bindings.unwrap_or(false)
  }

  pub fn is_on_demand_wrapping_enabled(&self) -> bool {
    self.on_demand_wrapping.unwrap_or(false)
  }

  #[inline]
  pub fn get_ignore_comment(&self) -> &'static str {
    if self.vite_mode.unwrap_or_default() { "@vite-ignore" } else { ROLLDOWN_IGNORE }
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

  pub fn is_chunk_import_map_enabled(&self) -> bool {
    self.chunk_import_map.unwrap_or(false)
  }
}
