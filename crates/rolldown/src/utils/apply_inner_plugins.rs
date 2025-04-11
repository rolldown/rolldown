use std::sync::Arc;

use rolldown_plugin::__inner::SharedPluginable;

/// Some builtin features of rolldown is implemented via plugins. However, though these features
/// are implemented via plugins, users could not feel the existence of these plugins. And to do so,
/// we need to apply these plugins after user's plugins to control the final order of plugins.
pub fn apply_inner_plugins(user_plugins: &mut Vec<SharedPluginable>) {
  user_plugins.push(Arc::new(rolldown_plugin_data_uri::DataUriPlugin::default()));
}
