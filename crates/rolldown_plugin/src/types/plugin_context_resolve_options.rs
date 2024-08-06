use rolldown_common::ImportKind;

#[derive(Debug)]
pub struct PluginContextResolveOptions {
  pub import_kind: ImportKind,
  pub skip_self: bool,
  /// The js side custom options is a complex js object, we can't directly convert it to rust struct. So here store a index to reference the js side custom options.
  pub custom: Option<u32>,
}

impl Default for PluginContextResolveOptions {
  fn default() -> Self {
    Self { import_kind: ImportKind::Import, skip_self: true, custom: None }
  }
}
