use std::sync::{Arc, Weak};

use dashmap::DashMap;
use oxc_index::IndexVec;
use rolldown_common::{SharedFileEmitter, SharedModuleInfoDashMap, SharedNormalizedBundlerOptions};
use rolldown_resolver::Resolver;
use rolldown_utils::dashmap::FxDashSet;

use crate::{
  __inner::SharedPluginable,
  PluginContext,
  plugin_context::{NativePluginContextImpl, PluginContextMeta},
  plugin_driver::{ContextLoadCompletionManager, hook_orders::PluginHookOrders},
  type_aliases::{IndexPluginContext, IndexPluginable},
};

pub struct PluginDriverFactory {
  plugins: Vec<SharedPluginable>,
  resolver: Arc<Resolver>,
}

impl PluginDriverFactory {
  pub fn new(plugins: Vec<SharedPluginable>, resolver: &Arc<Resolver>) -> Self {
    Self { plugins, resolver: Arc::clone(resolver) }
  }

  pub fn create_plugin_driver(
    &self,
    file_emitter: &SharedFileEmitter,
    options: &SharedNormalizedBundlerOptions,
    session: &rolldown_debug::Session,
    initial_bundle_span: &Arc<tracing::Span>,
    module_infos: SharedModuleInfoDashMap,
  ) -> Arc<crate::plugin_driver::PluginDriver> {
    let watch_files = Arc::new(FxDashSet::default());
    let meta = Arc::new(PluginContextMeta::default());
    let tx = Arc::new(tokio::sync::Mutex::new(None));
    let mut plugin_usage_vec = IndexVec::new();

    // Clone the Arc to share across contexts
    let bundle_span_arc = Arc::clone(initial_bundle_span);

    Arc::new_cyclic(|plugin_driver| {
      let mut index_plugins = IndexPluginable::with_capacity(self.plugins.len());
      let mut index_contexts = IndexPluginContext::with_capacity(self.plugins.len());

      self.plugins.iter().for_each(|plugin| {
        let plugin_idx = index_plugins.push(Arc::clone(plugin));
        plugin_usage_vec.push(plugin.call_hook_usage());
        index_contexts.push(PluginContext::Native(Arc::new(NativePluginContextImpl {
          plugin_name: plugin.call_name(),
          skipped_resolve_calls: vec![],
          plugin_idx,
          plugin_driver: Weak::clone(plugin_driver),
          meta: Arc::clone(&meta),
          resolver: Arc::clone(&self.resolver),
          file_emitter: Arc::clone(file_emitter),
          module_infos: Arc::clone(&module_infos),
          options: Arc::clone(options),
          watch_files: Arc::clone(&watch_files),
          tx: Arc::clone(&tx),
          session: session.clone(),
          bundle_span: Arc::clone(&bundle_span_arc),
        })));
      });

      crate::plugin_driver::PluginDriver {
        hook_orders: PluginHookOrders::new(&index_plugins, &plugin_usage_vec),
        plugins: index_plugins,
        contexts: index_contexts,
        file_emitter: Arc::clone(file_emitter),
        watch_files,
        module_infos,
        transform_dependencies: Arc::new(DashMap::default()),
        context_load_completion_manager: ContextLoadCompletionManager::default(),
        tx,
        options: Arc::clone(options),
      }
    })
  }
}
