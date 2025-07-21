use std::sync::Arc;

use rolldown_common::NormalizedBundlerOptions;
use rolldown_plugin::__inner::SharedPluginable;

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
) {
  let mut before_user_plugins: Vec<SharedPluginable> = vec![];

  if options.experimental.hmr.is_some() {
    before_user_plugins.push(Arc::new(rolldown_plugin_hmr::HmrPlugin));
  }

  if let Some(config) = &options.experimental.chunk_import_map {
    before_user_plugins.push(Arc::new(rolldown_plugin_chunk_import_map::ChunkImportMapPlugin {
      base_url: config.base_url.clone(),
      ..Default::default()
    }));
  }

  if !before_user_plugins.is_empty() {
    user_plugins.splice(0..0, before_user_plugins);
  }

  user_plugins.push(Arc::new(rolldown_plugin_data_uri::DataUriPlugin::default()));
}
