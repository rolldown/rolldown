use std::sync::Arc;

use rolldown_common::NormalizedBundlerOptions;
use rolldown_plugin::__inner::SharedPluginable;
use rolldown_plugin_lazy_compilation::LazyCompilationContext;

/// Result of applying inner plugins, containing optional contexts for features
pub struct ApplyInnerPluginsReturn {
  /// Context for lazy compilation, if enabled
  pub lazy_compilation_context: Option<LazyCompilationContext>,
}

/// Some builtin features of rolldown is implemented via builtin plugins. However, though these
/// features are implemented via plugins, users could not feel the existence of these plugins. And
/// to do so, we need to be careful about the order of plugins.
/// - These plugins should be always prepended to user's plugins, so they be decided whether to be
///   push forward or back via `PluginHookMeta`.
/// - Order of plugins theselves don't decide the final execution order of hooks.
/// - Control the order of plugins via `PluginHookMeta` mechanism.
pub fn apply_inner_plugins(
  options: &NormalizedBundlerOptions,
  user_plugins: &mut Vec<SharedPluginable>,
) -> ApplyInnerPluginsReturn {
  let mut before_user_plugins: Vec<SharedPluginable> = vec![
    Arc::new(rolldown_plugin_data_uri::DataUriPlugin::default()),
    Arc::new(rolldown_plugin_oxc_runtime::OxcRuntimePlugin),
  ];

  let mut lazy_compilation_context = None;

  if let Some(dev_mode) = &options.experimental.dev_mode {
    before_user_plugins.push(Arc::new(rolldown_plugin_hmr::HmrPlugin));
    if dev_mode.lazy == Some(true) {
      let plugin = rolldown_plugin_lazy_compilation::LazyCompilationPlugin::new();
      lazy_compilation_context = Some(plugin.context());
      before_user_plugins.push(Arc::new(plugin));
    }
  }

  if let Some(config) = &options.experimental.chunk_import_map {
    before_user_plugins.push(Arc::new(rolldown_plugin_chunk_import_map::ChunkImportMapPlugin {
      base_url: config.base_url.clone(),
      file_name: config.file_name.clone(),
      ..Default::default()
    }));
  }

  if !before_user_plugins.is_empty() {
    user_plugins.splice(0..0, before_user_plugins);
  }

  let copy_module_plugin =
    rolldown_plugin_copy_module::CopyModulePlugin::new(&options.module_types);
  if copy_module_plugin.is_active() {
    user_plugins.push(Arc::new(copy_module_plugin));
  }

  ApplyInnerPluginsReturn { lazy_compilation_context }
}
