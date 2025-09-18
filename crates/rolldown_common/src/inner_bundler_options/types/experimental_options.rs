#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

use crate::inner_bundler_options::types::chunk_import_map::ChunkImportMap;

use super::attach_debug_info::AttachDebugInfo;
use super::chunk_modules_order::ChunkModulesOrderBy;
use super::hmr_options::HmrOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
#[cfg_attr(feature = "deserialize_bundler_options", serde(rename_all = "camelCase"))]
pub enum SourcemapHires {
  #[cfg_attr(feature = "deserialize_bundler_options", schemars(with = "bool"))]
  Boolean(bool),
  Boundary,
}

impl From<SourcemapHires> for string_wizard::Hires {
  fn from(value: SourcemapHires) -> Self {
    match value {
      SourcemapHires::Boolean(value) => {
        if value {
          string_wizard::Hires::True
        } else {
          string_wizard::Hires::False
        }
      }
      SourcemapHires::Boundary => string_wizard::Hires::Boundary,
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
  pub strict_execution_order: Option<bool>,
  pub disable_live_bindings: Option<bool>,
  pub vite_mode: Option<bool>,
  pub resolve_new_url_to_asset: Option<bool>,
  pub incremental_build: Option<bool>,
  pub hmr: Option<HmrOptions>,
  pub attach_debug_info: Option<AttachDebugInfo>,
  pub chunk_import_map: Option<ChunkImportMap>,
  pub chunk_modules_order: Option<ChunkModulesOrderBy>,
  pub on_demand_wrapping: Option<bool>,
  pub transform_hires_sourcemap: Option<SourcemapHires>,
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
}
