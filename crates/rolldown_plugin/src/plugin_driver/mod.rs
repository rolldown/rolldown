use std::sync::{Arc, Weak};

use crate::{
  plugin_context::SharedPluginContext, worker_manager::WorkerManager, PluginContext,
  PluginOrThreadSafePlugin,
};

mod build_hooks;
mod output_hooks;

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  plugins: Vec<(PluginOrThreadSafePlugin, SharedPluginContext)>,
  worker_manager: Option<WorkerManager>,
}

impl PluginDriver {
  pub fn new_shared(
    plugins: Vec<PluginOrThreadSafePlugin>,
    worker_count: u16,
  ) -> SharedPluginDriver {
    Arc::new_cyclic(|plugin_driver| {
      let with_context = plugins
        .into_iter()
        .map(|plugin| (plugin, PluginContext { _plugin_driver: Weak::clone(plugin_driver) }.into()))
        .collect::<Vec<_>>();

      let worker_manager =
        if worker_count > 0 { Some(WorkerManager::new(worker_count)) } else { None };

      Self { plugins: with_context, worker_manager }
    })
  }
}
