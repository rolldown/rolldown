#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

use crate::ROLLDOWN_IGNORE;

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
}

impl ExperimentalOptions {
  pub fn is_strict_execution_order_enabled(&self) -> bool {
    self.strict_execution_order.unwrap_or(false)
  }

  pub fn is_disable_live_bindings_enabled(&self) -> bool {
    self.disable_live_bindings.unwrap_or(false)
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
}
