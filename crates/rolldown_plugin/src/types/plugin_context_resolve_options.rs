use rolldown_common::ImportKind;

#[derive(Debug)]
pub struct PluginContextResolveOptions {
  pub import_kind: ImportKind,
  pub skip_self: bool,
}

impl Default for PluginContextResolveOptions {
  fn default() -> Self {
    Self { import_kind: ImportKind::Import, skip_self: true }
  }
}
