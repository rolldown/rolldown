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
  ModuleId, ModuleIdx, ModuleInfo, ModuleLoaderMsg, PluginIdx, SharedFileEmitter,
  SharedModuleInfoDashMap, SharedNormalizedBundlerOptions,
};
use rolldown_resolver::Resolver;
use rolldown_utils::dashmap::FxDashSet;
use sugar_path::SugarPath;
use tokio::sync::{Mutex, broadcast};

use crate::{
  __inner::SharedPluginable,
  PluginContext,
  plugin_context::{NativePluginContextImpl, PluginContextMeta},
  plugin_driver::hook_orders::PluginHookOrders,
  type_aliases::{IndexPluginContext, IndexPluginable},
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
  /// Transform dependencies per module, tracked during transform hooks
  pub transform_dependencies: Arc<DashMap<ModuleIdx, Arc<FxDashSet<ArcStr>>>>,
  context_load_completion_manager: ContextLoadCompletionManager,
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
        transform_dependencies: Arc::new(DashMap::default()),
        context_load_completion_manager: ContextLoadCompletionManager::default(),
        tx,
        options: Arc::clone(options),
      }
    })
  }

  pub fn clear(&self) {
    self.watch_files.clear();
    self.modules.clear();
    self.transform_dependencies.clear();
    self.context_load_completion_manager.clear();
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

  pub fn mark_context_load_modules_loaded(&self, module_id: ModuleId) {
    self.context_load_completion_manager.mark_completion(module_id);
  }

  pub fn invalidate_context_load_module(&self, module_id: &ModuleId) {
    self.context_load_completion_manager.invalidate(module_id);
  }

  pub async fn wait_for_module_load_completion(&self, specifier: &str) {
    self.context_load_completion_manager.wait_for_completion(specifier.into()).await;
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

  pub fn add_transform_dependency(&self, module_idx: ModuleIdx, dependency: &str) {
    let dependency = ArcStr::from(dependency.to_slash().unwrap());

    self
      .transform_dependencies
      .entry(module_idx)
      .or_insert_with(|| Arc::new(FxDashSet::default()))
      .insert(dependency);
  }
}

impl Deref for PluginDriver {
  type Target = PluginHookOrders;
  fn deref(&self) -> &Self::Target {
    &self.hook_orders
  }
}

#[derive(Default)]
struct ContextLoadCompletionManager {
  notifiers: DashMap<ModuleId, ContextLoadCompletionState>,
}

enum ContextLoadCompletionState {
  Pending(broadcast::Sender<()>),
  Completed,
}

impl ContextLoadCompletionManager {
  pub async fn wait_for_completion(&self, module_id: ModuleId) {
    let mut rx = match self.notifiers.entry(module_id) {
      dashmap::Entry::Vacant(guard) => {
        let (tx, rx) = broadcast::channel(1);
        guard.insert(ContextLoadCompletionState::Pending(tx));
        rx
      }
      dashmap::Entry::Occupied(mut guard) => match guard.get_mut() {
        ContextLoadCompletionState::Pending(sender) => sender.subscribe(),
        ContextLoadCompletionState::Completed => {
          /* no need to wait */
          return;
        }
      },
    };

    if let Err(err) = rx.recv().await {
      // This happens when `.invalidate` is called before `.mark_completion` is called, which is not expected
      debug_assert!(
        false,
        "The sender was dropped while waiting for module load completion: {err}"
      );
      tracing::warn!("The sender was dropped while waiting for module load completion");
    }
  }

  pub fn mark_completion(&self, module_id: ModuleId) {
    match self.notifiers.entry(module_id) {
      dashmap::Entry::Vacant(guard) => {
        guard.insert(ContextLoadCompletionState::Completed);
      }
      dashmap::Entry::Occupied(mut guard) => match guard.get_mut() {
        ContextLoadCompletionState::Pending(sender) => {
          sender.send(()).expect(
            "PluginDriver: failed to send completion notification - receiver was dropped before wait_for_completion was called, indicating a race condition in module loading"
          );
          *guard.get_mut() = ContextLoadCompletionState::Completed;
        }
        ContextLoadCompletionState::Completed => {
          // This happens if `.mark_completion` is called multiple times, which is not expected
          debug_assert!(false, "mark_completion was called even though it was already completed");
          tracing::warn!("mark_completion was called even though it was already completed");
        }
      },
    }
  }

  pub fn invalidate(&self, module_id: &ModuleId) {
    self.notifiers.remove(module_id);
  }

  pub fn clear(&self) {
    self.notifiers.clear();
  }
}
