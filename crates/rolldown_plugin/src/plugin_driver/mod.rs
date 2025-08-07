mod build_hooks;
mod hook_orders;
mod output_hooks;
mod watch_hooks;

use std::{
  ops::Deref,
  sync::{Arc, Weak},
  vec,
};

use arcstr::ArcStr;
use dashmap::{DashMap, DashSet};
use oxc_index::IndexVec;
use rolldown_common::{
  ModuleId, ModuleInfo, ModuleLoaderMsg, SharedFileEmitter, SharedModuleInfoDashMap,
  SharedNormalizedBundlerOptions,
};
use rolldown_resolver::Resolver;
use rolldown_utils::dashmap::FxDashSet;
use tokio::sync::Mutex;

use crate::{
  __inner::SharedPluginable,
  PluginContext,
  plugin_context::{NativePluginContextImpl, PluginContextMeta},
  plugin_driver::hook_orders::PluginHookOrders,
  type_aliases::{IndexPluginContext, IndexPluginable},
  types::plugin_idx::PluginIdx,
};

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  plugins: IndexPluginable,
  contexts: IndexPluginContext,
  hook_orders: PluginHookOrders,
  options: SharedNormalizedBundlerOptions,
  pub file_emitter: SharedFileEmitter,
  pub watch_files: Arc<FxDashSet<ArcStr>>,
  pub modules: SharedModuleInfoDashMap,
  pub(crate) tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<ModuleLoaderMsg>>>>,
}

impl PluginDriver {
  pub fn new_shared(
    plugins: Vec<SharedPluginable>,
    resolver: &Arc<Resolver>,
    file_emitter: &SharedFileEmitter,
    options: &SharedNormalizedBundlerOptions,
  ) -> SharedPluginDriver {
    let watch_files = Arc::new(DashSet::default());
    let modules = Arc::new(DashMap::default());
    let meta = Arc::new(PluginContextMeta::default());
    let tx = Arc::new(Mutex::new(None));
    let mut plugin_usage_vec = IndexVec::new();

    Arc::new_cyclic(|plugin_driver| {
      let mut index_plugins = IndexPluginable::with_capacity(plugins.len());
      let mut index_contexts = IndexPluginContext::with_capacity(plugins.len());

      plugins.into_iter().for_each(|plugin| {
        let plugin_idx = index_plugins.push(Arc::clone(&plugin));
        plugin_usage_vec.push(plugin.call_hook_usage());
        index_contexts.push(PluginContext::Native(Arc::new(NativePluginContextImpl {
          plugin_name: plugin.call_name(),
          skipped_resolve_calls: vec![],
          plugin_idx,
          plugin_driver: Weak::clone(plugin_driver),
          meta: Arc::clone(&meta),
          resolver: Arc::clone(resolver),
          file_emitter: Arc::clone(file_emitter),
          modules: Arc::clone(&modules),
          options: Arc::clone(options),
          watch_files: Arc::clone(&watch_files),
          tx: Arc::clone(&tx),
        })));
      });

      Self {
        hook_orders: PluginHookOrders::new(&index_plugins, &plugin_usage_vec),
        plugins: index_plugins,
        contexts: index_contexts,
        file_emitter: Arc::clone(file_emitter),
        watch_files,
        modules,
        tx,
        options: Arc::clone(options),
      }
    })
  }

  pub fn clear(&self) {
    self.watch_files.clear();
    self.modules.clear();
    self.file_emitter.clear();
  }

  pub fn set_module_info(&self, module_id: &ModuleId, module_info: Arc<ModuleInfo>) {
    self.modules.insert(module_id.resource_id().into(), module_info);
  }

  pub async fn set_context_load_modules_tx(
    &self,
    tx: Option<tokio::sync::mpsc::Sender<ModuleLoaderMsg>>,
  ) {
    let mut tx_guard = self.tx.lock().await;
    *tx_guard = tx;
  }

  pub async fn mark_context_load_modules_loaded(
    &self,
    module_id: &ModuleId,
    success: bool,
  ) -> anyhow::Result<()> {
    if let Some(mark_module_loaded) = &self.options.mark_module_loaded {
      mark_module_loaded.call(module_id, success).await?;
    }
    Ok(())
  }

  pub fn iter_plugin_with_context_by_order<'me>(
    &'me self,
    ordered_plugins: &'me [PluginIdx],
  ) -> impl Iterator<Item = (PluginIdx, &'me SharedPluginable, &'me PluginContext)> + 'me {
    ordered_plugins.iter().copied().map(move |idx| {
      let plugin = &self.plugins[idx];
      let context = &self.contexts[idx];
      (idx, plugin, context)
    })
  }

  pub fn plugins(&self) -> &IndexPluginable {
    &self.plugins
  }
}

impl Deref for PluginDriver {
  type Target = PluginHookOrders;
  fn deref(&self) -> &Self::Target {
    &self.hook_orders
  }
}
