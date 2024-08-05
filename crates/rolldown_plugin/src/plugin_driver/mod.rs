use std::sync::{Arc, Weak};

use rolldown_common::{ModuleTable, SharedFileEmitter};
use rolldown_resolver::Resolver;

use crate::{__inner::SharedPluginable, plugin_context::SharedPluginContext, PluginContext};

mod build_hooks;
mod output_hooks;

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  plugins: Vec<(SharedPluginable, SharedPluginContext)>,
}

impl PluginDriver {
  pub fn new_shared(
    plugins: Vec<SharedPluginable>,
    resolver: &Arc<Resolver>,
    file_emitter: &SharedFileEmitter,
  ) -> SharedPluginDriver {
    Arc::new_cyclic(|plugin_driver| {
      let with_context = plugins
        .into_iter()
        .map(|plugin| {
          (
            Arc::clone(&plugin),
            PluginContext {
              skipped_resolve_calls: vec![],
              plugin: Arc::clone(&plugin),
              plugin_driver: Weak::clone(plugin_driver),
              resolver: Arc::clone(resolver),
              file_emitter: Arc::clone(file_emitter),
              module_table: None,
            }
            .into(),
          )
        })
        .collect::<Vec<_>>();

      Self { plugins: with_context }
    })
  }

  pub fn new_shared_with_module_table(
    &self,
    module_table: &Arc<&'static mut ModuleTable>,
  ) -> SharedPluginDriver {
    Arc::new_cyclic(|plugin_driver| {
      let with_context = self
        .plugins
        .iter()
        .map(|(plugin, ctx)| {
          (
            Arc::clone(plugin),
            PluginContext {
              skipped_resolve_calls: vec![],
              plugin: Arc::clone(plugin),
              plugin_driver: Weak::clone(plugin_driver),
              resolver: Arc::clone(&ctx.resolver),
              file_emitter: Arc::clone(&ctx.file_emitter),
              module_table: Some(Arc::clone(module_table)),
            }
            .into(),
          )
        })
        .collect::<Vec<_>>();

      Self { plugins: with_context }
    })
  }
}
