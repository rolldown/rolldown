use std::sync::Arc;

use rolldown_common::ImportKind;

use crate::Plugin;

#[derive(Debug, Clone)]
pub struct HookResolveIdExtraOptions {
  pub is_entry: bool,
  // Rollup hasn't this filed, but since Rolldown support cjs as first citizen, so we need to generate `kind` to distinguish it.
  pub kind: ImportKind,
  pub skip_plugin: Option<Arc<dyn Plugin>>,
}
