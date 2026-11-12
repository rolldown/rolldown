#[derive(Debug, PartialEq, Eq)]
pub enum PluginOrder {
  Pre,
  Post,
  /// Runs after all `Post` hooks. Unlike `Post`, earlier plugins run later —
  /// so a plugin registered first is guaranteed to run last.
  PinPost,
}

#[derive(Debug, Default)]
pub struct PluginHookMeta {
  pub order: Option<PluginOrder>,
  /// Whether Rolldown may skip this hook after a restart and reuse its saved result.
  /// Only `resolve_id`, `resolve_dynamic_import`, `load` and `transform` read it.
  /// `None` lets `experimental.cache.assumeCacheableHooks` decide. `Some(false)` never caches.
  pub cache: Option<bool>,
}
