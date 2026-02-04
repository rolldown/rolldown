use std::sync::{Arc, Weak};

use arcstr::ArcStr;
use dashmap::DashMap;
use oxc_index::IndexVec;
use rolldown_common::{
  ModuleIdx, SharedFileEmitter, SharedModuleInfoDashMap, SharedNormalizedBundlerOptions,
};
use rolldown_resolver::Resolver;
use rolldown_utils::dashmap::FxDashSet;

use crate::{
  __inner::SharedPluginable,
  PluginContext,
  plugin_context::{NativePluginContextImpl, PluginContextMeta},
  plugin_driver::{ContextLoadCompletionManager, hook_orders::PluginHookOrders},
  type_aliases::{IndexPluginContext, IndexPluginable},
  types::hook_timing::HookTimingCollector,
};
use rolldown_error::EventKindSwitcher;

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
    session: &rolldown_devtools::Session,
    initial_bundle_span: &Arc<tracing::Span>,
    module_infos: SharedModuleInfoDashMap,
    transform_dependencies: Arc<DashMap<ModuleIdx, Arc<FxDashSet<ArcStr>>>>,
  ) -> Arc<crate::plugin_driver::PluginDriver> {
    let watch_files = Arc::new(FxDashSet::default());
    let meta = Arc::new(PluginContextMeta::default());
    let tx = Arc::new(tokio::sync::Mutex::new(None));
    let mut plugin_usage_vec = IndexVec::new();

    // Clone the Arc to share across contexts
    let bundle_span_arc = Arc::clone(initial_bundle_span);

    // Create derived span for manual resolve calls
    let manual_resolve_span_arc = Arc::new(tracing::debug_span!(
      parent: bundle_span_arc.as_ref(),
      "plugin_context_resolve",
      CONTEXT_hook_resolve_id_trigger = "manual"
    ));

    // Create timing collector only if checks.pluginTimings is enabled
    let hook_timing_collector = if options.checks.contains(EventKindSwitcher::PluginTimings) {
      Some(Arc::new(HookTimingCollector::default()))
    } else {
      None
    };

    Arc::new_cyclic(|plugin_driver| {
      let mut index_plugins = IndexPluginable::with_capacity(self.plugins.len());
      let mut index_contexts = IndexPluginContext::with_capacity(self.plugins.len());

      self.plugins.iter().for_each(|plugin| {
        let plugin_idx = index_plugins.push(Arc::clone(plugin));
        plugin_usage_vec.push(plugin.call_hook_usage());

        // Register plugin in timing collector if enabled (skip internal builtin plugins)
        let plugin_name = plugin.call_name();
        if let Some(ref collector) = hook_timing_collector {
          if !plugin_name.starts_with("builtin:") {
            collector.register_plugin(plugin_idx, ArcStr::from(plugin_name.as_ref()));
          }
        }

        index_contexts.push(PluginContext::Native(Arc::new(NativePluginContextImpl {
          plugin_name,
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
          manual_resolve_span: Arc::clone(&manual_resolve_span_arc),
        })));
      });

      crate::plugin_driver::PluginDriver {
        hook_orders: PluginHookOrders::new(&index_plugins, &plugin_usage_vec),
        plugins: index_plugins,
        contexts: index_contexts,
        file_emitter: Arc::clone(file_emitter),
        watch_files,
        module_infos,
        transform_dependencies,
        context_load_completion_manager: ContextLoadCompletionManager::default(),
        tx,
        hook_timing_collector: hook_timing_collector.clone(),
      }
    })
  }
}
