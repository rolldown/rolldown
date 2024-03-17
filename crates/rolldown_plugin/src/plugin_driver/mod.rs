use std::sync::{Arc, Weak};

use crate::{BoxPlugin, PluginContext};

mod build_hooks;
mod output_hooks;

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  plugins: Vec<(BoxPlugin, PluginContext)>,
}

impl PluginDriver {
  pub fn new_shared(plugins: Vec<BoxPlugin>) -> SharedPluginDriver {
    Arc::new_cyclic(|plugin_driver| {
      let with_context = plugins
        .into_iter()
        .map(|plugin| (plugin, PluginContext { _plugin_driver: Weak::clone(plugin_driver) }))
        .collect::<Vec<_>>();

      Self { plugins: with_context }
    })
  }
}
