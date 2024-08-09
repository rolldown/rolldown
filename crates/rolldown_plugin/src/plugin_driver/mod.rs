use std::{
  sync::{Arc, Weak},
  vec,
};

use rolldown_common::{ModuleTable, SharedFileEmitter};
use rolldown_resolver::Resolver;

use crate::{
  __inner::SharedPluginable,
  type_aliases::{IndexPluginContext, IndexPluginable},
  types::plugin_idx::PluginIdx,
  PluginContext, PluginHookMeta, PluginOrder, SharedPluginContext,
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

  pub fn iter_sorted_plugin_with_context(
    &self,
    meta_fn: impl Fn(&SharedPluginable) -> Option<PluginHookMeta>,
  ) -> impl Iterator<Item = (PluginIdx, &SharedPluginable, &SharedPluginContext)> {
    let mut pre_plugins_with_contexts = vec![];
    let mut normal_plugins_with_contexts = vec![];
    let mut post_plugins_with_contexts = vec![];

    for (idx, plugin, context, meta) in self
      .plugins
      .iter_enumerated()
      .zip(self.contexts.iter())
      .map(|((index, plugin), context)| (index, plugin, context, meta_fn(plugin)))
    {
      if let Some(meta) = meta {
        match meta.order {
          Some(PluginOrder::Pre) => pre_plugins_with_contexts.push((idx, plugin, context)),
          Some(PluginOrder::Post) => post_plugins_with_contexts.push((idx, plugin, context)),
          None => normal_plugins_with_contexts.push((idx, plugin, context)),
        }
      } else {
        normal_plugins_with_contexts.push((idx, plugin, context));
      }
    }

    pre_plugins_with_contexts
      .into_iter()
      .chain(normal_plugins_with_contexts)
      .chain(post_plugins_with_contexts)
  }
}
