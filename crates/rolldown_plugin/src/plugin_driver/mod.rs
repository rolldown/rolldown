mod build_hooks;
mod hook_orders;
mod output_hooks;
mod plugin_driver_factory;
mod watch_hooks;

pub use plugin_driver_factory::PluginDriverFactory;

use std::{
  ops::Deref,
  sync::{Arc, Mutex},
};

use anyhow::Context;
use arcstr::ArcStr;
use dashmap::DashMap;
use rolldown_common::{
  ModuleDefFormat, ModuleId, ModuleIdx, ModuleInfo, ModuleLoaderMsg, PluginIdx,
  SharedFileEmitter, SharedModuleInfoDashMap, WatchPath,
};
use rolldown_utils::dashmap::FxDashSet;
use tokio::sync::broadcast;

use crate::{
  __inner::SharedPluginable,
  PluginContext,
  plugin_driver::hook_orders::PluginHookOrders,
  type_aliases::{IndexPluginContext, IndexPluginable},
  types::build_timings::BuildTimings,
};

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  plugins: IndexPluginable,
  contexts: IndexPluginContext,
  hook_orders: PluginHookOrders,
  pub(crate) should_skip_user_plugins_for_lazy_proxy_modules: bool,
  pub(crate) lazy_compilation_plugin_idx: Option<PluginIdx>,
  pub file_emitter: SharedFileEmitter,
  pub watch_files: Arc<FxDashSet<ArcStr>>,
  pub module_infos: SharedModuleInfoDashMap,
  /// Module dependencies tracked during load/transform hooks for HMR invalidation
  pub transform_dependencies: Arc<DashMap<ModuleIdx, Arc<FxDashSet<WatchPath>>>>,
  context_load_completion_manager: ContextLoadCompletionManager,
  pub(crate) tx: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<ModuleLoaderMsg>>>>,
  /// Outlives the `Bundle` through `BundleHandle`, which is how the binding reads them
  /// once the build has finished.
  pub build_timings: BuildTimings,
}

impl PluginDriver {
  pub fn clear(&self) {
    self.watch_files.clear();
    self.module_infos.clear();
    // Note: transform_dependencies is NOT cleared here - it's preserved across incremental builds
    // by BundleFactory which manages its lifecycle (reset on full builds only)
    self.context_load_completion_manager.clear();
    self.file_emitter.clear();
  }

  pub fn set_module_info(&self, module_id: &ModuleId, module_info: Arc<ModuleInfo>) {
    self.module_infos.insert(module_id.as_arc_str().into(), module_info);
  }

  pub fn set_context_load_modules_tx(
    &self,
    tx: Option<tokio::sync::mpsc::UnboundedSender<ModuleLoaderMsg>>,
  ) -> anyhow::Result<()> {
    *self.tx.lock().ok().context("Failed to acquire PluginDriver tx lock")? = tx;
    Ok(())
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
  ) -> impl ExactSizeIterator<Item = (PluginIdx, &'me SharedPluginable, &'me PluginContext)> + 'me
  {
    ordered_plugins.iter().copied().map(move |idx| {
      let plugin = &self.plugins[idx];
      let context = &self.contexts[idx];
      (idx, plugin, context)
    })
  }

  pub fn plugins(&self) -> &IndexPluginable {
    &self.plugins
  }

  /// developer-bundler: the format whose CommonJS interop the module `id` follows, in place of
  /// `format`, its own; see `Plugin::interop_module_def_format`.
  pub fn interop_module_def_format(&self, id: &str, format: ModuleDefFormat) -> ModuleDefFormat {
    self
      .plugins
      .iter()
      .find_map(|plugin| plugin.call_interop_module_def_format(id, format))
      .unwrap_or(format)
  }

  pub fn add_transform_dependency(&self, module_idx: ModuleIdx, dependency: WatchPath) {
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

/// The `CONTEXT_call_id` of a hook-call span at `target`, or `None` when nothing can observe the
/// span. Once a dispatcher has been set (globally or scoped), tracing evaluates a span's fields
/// only for an enabled span, so the id is always generated then. Before that, tracing's `log`
/// fallback evaluates them for every span and drops the record unless the `log` logger is enabled
/// at `TRACE` for the span's target, so the id is generated only in that case. Every observer
/// therefore receives the same field it did with an unconditional id, without a UUID per hook
/// call that no one reads.
#[doc(hidden)]
pub fn hook_call_id(target: &str) -> Option<String> {
  (tracing::dispatcher::has_been_set() || log::log_enabled!(target: target, log::Level::Trace))
    .then(rolldown_utils::uuid::uuid_v4)
}
