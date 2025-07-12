use std::sync::Arc;

use rolldown_common::ImportKind;

use super::custom_field::CustomField;

#[derive(Debug)]
pub struct PluginContextResolveOptions {
  pub import_kind: ImportKind,
  pub is_entry: bool,
  pub skip_self: bool,
  pub custom: Arc<CustomField>,
}

impl Default for PluginContextResolveOptions {
  fn default() -> Self {
    Self {
      import_kind: ImportKind::Import,
      is_entry: false,
      skip_self: true,
      custom: Arc::default(),
    }
  }
}
