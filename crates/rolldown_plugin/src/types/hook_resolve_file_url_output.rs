use std::borrow::Cow;

/// A plugin's `resolveFileUrl` result.
///
/// The code is not parsed here. It is parsed once, later, into the arena of the module
/// that references the file. `plugin_name` travels with it so a parse failure at that
/// point can still name the plugin that produced the code.
#[derive(Debug)]
pub struct HookResolveFileUrlOutput {
  /// A single JavaScript expression, replacing `import.meta.ROLLUP_FILE_URL_<id>`.
  pub code: String,
  pub plugin_name: Cow<'static, str>,
}
