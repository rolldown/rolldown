use super::native_lib_plugin::NativeLibPlugin;

/// JS-facing descriptor for a native plugin loaded via the
/// `rolldown_native_plugin_abi` C ABI. The plugin's `.dylib`/`.so`/`.dll` at
/// `path` must export the three required symbols
/// (`rolldown_native_plugin_abi_version`, `rolldown_native_plugin_transform`,
/// `rolldown_native_plugin_drop_output`).
///
/// Plugins are loaded once at bundle setup and dispatched directly from
/// rolldown's worker threads — no napi, no JS thread, no `ThreadsafeFunction`.
#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingNativeLibPlugin {
  /// Display name (used in diagnostics and telemetry).
  pub name: String,
  /// Filesystem path to the plugin shared library.
  pub path: String,
}

impl TryFrom<BindingNativeLibPlugin> for NativeLibPlugin {
  type Error = napi::Error;

  fn try_from(value: BindingNativeLibPlugin) -> Result<Self, Self::Error> {
    NativeLibPlugin::load(value.name, &value.path)
  }
}
