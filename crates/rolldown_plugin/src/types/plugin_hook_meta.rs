#[derive(Debug, PartialEq, Eq)]
pub enum PluginOrder {
  Pre,
  Post,
  /// Runs after all `Post` hooks. Unlike `Post`, earlier plugins run later —
  /// so a plugin registered first is guaranteed to run last.
  PinPost,
}

#[derive(Debug)]
pub struct PluginHookMeta {
  pub order: Option<PluginOrder>,
}
