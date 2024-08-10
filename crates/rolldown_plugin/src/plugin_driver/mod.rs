use std::sync::{Arc, Weak};

use rolldown_common::{ModuleTable, SharedFileEmitter};
use rolldown_resolver::Resolver;
use rolldown_stats::Stats;

use crate::{
  __inner::SharedPluginable,
  type_aliases::{IndexPluginContext, IndexPluginable},
  types::plugin_idx::PluginIdx,
  PluginContext, SharedPluginContext,
};

mod build_hooks;
mod output_hooks;

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  plugins: IndexPluginable,
  contexts: IndexPluginContext,
}

impl PluginDriver {
  pub fn new_shared(
    plugins: Vec<SharedPluginable>,
    resolver: &Arc<Resolver>,
    file_emitter: &SharedFileEmitter,
    stats: &Arc<Stats>,
  ) -> SharedPluginDriver {
    Arc::new_cyclic(|plugin_driver| {
      let mut index_plugins = IndexPluginable::with_capacity(plugins.len());
      let mut index_contexts = IndexPluginContext::with_capacity(plugins.len());

      plugins.into_iter().for_each(|plugin| {
        let plugin_idx = index_plugins.push(Arc::clone(&plugin));
        index_contexts.push(
          PluginContext {
            skipped_resolve_calls: vec![],
            plugin_idx,
            plugin_driver: Weak::clone(plugin_driver),
            resolver: Arc::clone(resolver),
            file_emitter: Arc::clone(file_emitter),
            module_table: None,
          }
          .into(),
        );
      });

      Self { plugins: index_plugins, contexts: index_contexts }
    })
  }

  pub fn new_shared_with_module_table(
    &self,
    module_table: &Arc<&'static mut ModuleTable>,
  ) -> SharedPluginDriver {
    Arc::new_cyclic(|plugin_driver| {
      let mut index_plugins = IndexPluginable::with_capacity(self.plugins.len());
      let mut index_contexts = IndexPluginContext::with_capacity(self.plugins.len());

      self.plugins.iter().zip(self.contexts.iter()).for_each(|(plugin, ctx)| {
        let plugin_idx = index_plugins.push(Arc::clone(plugin));
        index_contexts.push(
          PluginContext {
            skipped_resolve_calls: vec![],
            plugin_idx,
            plugin_driver: Weak::clone(plugin_driver),
            resolver: Arc::clone(&ctx.resolver),
            file_emitter: Arc::clone(&ctx.file_emitter),
            module_table: Some(Arc::clone(module_table)),
          }
          .into(),
        );
      });

      Self { plugins: index_plugins, contexts: index_contexts }
    })
  }

  pub(crate) fn iter_plugin_with_context(
    &self,
  ) -> impl Iterator<Item = (&SharedPluginable, &SharedPluginContext)> {
    self.plugins.iter().zip(self.contexts.iter())
  }

  pub(crate) fn iter_enumerated_plugin_with_context(
    &self,
  ) -> impl Iterator<Item = (PluginIdx, &SharedPluginable, &SharedPluginContext)> {
    self
      .plugins
      .iter_enumerated()
      .zip(self.contexts.iter())
      .map(|((idx, plugin), ctx)| (idx, plugin, ctx))
  }
}
