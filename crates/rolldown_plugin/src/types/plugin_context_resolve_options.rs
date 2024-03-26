use rolldown_common::ImportKind;

pub struct PluginContextResolveOptions {
  pub import_kind: ImportKind,
}

impl Default for PluginContextResolveOptions {
  fn default() -> Self {
    Self { import_kind: ImportKind::Import }
  }
}
