use std::sync::Arc;

use rolldown_common::ImportKind;
use typedmap::TypedDashMap;

#[derive(Debug)]
pub struct PluginContextResolveOptions {
  pub import_kind: ImportKind,
  pub skip_self: bool,
  pub custom: Arc<TypedDashMap>,
}

impl Default for PluginContextResolveOptions {
  fn default() -> Self {
    Self { import_kind: ImportKind::Import, skip_self: true, custom: Arc::default() }
  }
}
