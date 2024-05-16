use std::sync::{Arc, Weak};

use rolldown_common::SharedModuleTable;
use rolldown_resolver::Resolver;

use crate::{plugin_context::SharedPluginContext, BoxPlugin, PluginContext};

mod build_hooks;
mod output_hooks;

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  plugins: Vec<(BoxPlugin, SharedPluginContext)>,
}

impl PluginDriver {
  pub fn new_shared(
    plugins: Vec<BoxPlugin>,
    resolver: &Arc<Resolver>,
    module_table: &SharedModuleTable,
  ) -> SharedPluginDriver {
    Arc::new_cyclic(|plugin_driver| {
      let with_context = plugins
        .into_iter()
        .map(|plugin| {
          (
            plugin,
            PluginContext {
              plugin_driver: Weak::clone(plugin_driver),
              resolver: Arc::clone(resolver),
              module_table: Arc::clone(module_table),
            }
            .into(),
          )
        })
        .collect::<Vec<_>>();

      Self { plugins: with_context }
    })
  }
}
