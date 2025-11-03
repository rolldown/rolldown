use rolldown_common::PluginIdx;
use rolldown_utils::dashmap::FxDashMap;
use std::sync::Arc;

use super::FilterExprCache;

/// Storage for plugin filter overrides that can be set after plugin creation
#[derive(Debug, Default)]
pub struct PluginFilterOverrides {
  inner: FxDashMap<PluginIdx, Arc<FilterExprCache>>,
}

impl PluginFilterOverrides {
  pub fn set(&self, plugin_idx: PluginIdx, cache: FilterExprCache) {
    self.inner.insert(plugin_idx, Arc::new(cache));
  }

  pub fn get(&self, plugin_idx: PluginIdx) -> Option<Arc<FilterExprCache>> {
    self.inner.get(&plugin_idx).map(|v| Arc::clone(&v))
  }
}
