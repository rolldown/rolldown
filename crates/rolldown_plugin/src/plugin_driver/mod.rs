mod build_hooks;
mod hook_orders;
mod output_hooks;
mod plugin_driver_factory;
mod watch_hooks;

pub use plugin_driver_factory::PluginDriverFactory;

use std::{ops::Deref, sync::Arc};

use arcstr::ArcStr;
use dashmap::DashMap;
use rolldown_common::{
  ModuleId, ModuleIdx, ModuleInfo, ModuleLoaderMsg, PluginIdx, SharedFileEmitter,
  SharedModuleInfoDashMap,
};
use rolldown_utils::dashmap::FxDashSet;
use sugar_path::SugarPath;
use tokio::sync::{Mutex, broadcast};

use crate::{
  __inner::SharedPluginable,
  PluginContext,
  plugin_driver::hook_orders::PluginHookOrders,
  type_aliases::{IndexPluginContext, IndexPluginable},
  types::hook_timing::HookTimingCollector,
};

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  plugins: IndexPluginable,
  contexts: IndexPluginContext,
  hook_orders: PluginHookOrders,
  pub file_emitter: SharedFileEmitter,
  pub watch_files: Arc<FxDashSet<ArcStr>>,
  pub module_infos: SharedModuleInfoDashMap,
  /// Module dependencies tracked during load/transform hooks for HMR invalidation
  pub transform_dependencies: Arc<DashMap<ModuleIdx, Arc<FxDashSet<ArcStr>>>>,
  context_load_completion_manager: ContextLoadCompletionManager,
  pub(crate) tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<ModuleLoaderMsg>>>>,
  /// Timing collector for plugin hooks (None if plugin timing is disabled)
  pub hook_timing_collector: Option<Arc<HookTimingCollector>>,
}

impl PluginDriver {
  pub fn clear(&self) {
    self.watch_files.clear();
    self.module_infos.clear();
    // Note: transform_dependencies is NOT cleared here - it's preserved across incremental builds
    // by BundleFactory which manages its lifecycle (reset on full builds only)
    self.context_load_completion_manager.clear();
    self.file_emitter.clear();
    if let Some(collector) = &self.hook_timing_collector {
      collector.clear();
    }
  }

  pub fn set_module_info(&self, module_id: &ModuleId, module_info: Arc<ModuleInfo>) {
    self.module_infos.insert(module_id.as_arc_str().into(), module_info);
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

  /// Record hook timing if timing collection is enabled.
  /// Returns `Some(Instant)` if timing is enabled, `None` otherwise.
  #[inline]
  #[must_use]
  pub fn start_timing(&self) -> Option<std::time::Instant> {
    self.hook_timing_collector.as_ref().map(|_| std::time::Instant::now())
  }

  /// Record the elapsed time for a plugin if timing collection is enabled.
  #[inline]
  pub fn record_timing(&self, plugin_idx: PluginIdx, start: Option<std::time::Instant>) {
    if let (Some(collector), Some(start)) = (&self.hook_timing_collector, start) {
      #[expect(clippy::cast_possible_truncation)]
      collector.record(plugin_idx, start.elapsed().as_micros() as u64);
    }
  }

  /// Set total build time from start instant
  #[inline]
  pub fn set_total_build_time(&self, start: Option<std::time::Instant>) {
    if let (Some(collector), Some(start)) = (&self.hook_timing_collector, start) {
      #[expect(clippy::cast_possible_truncation)]
      collector.set_total_build_micros(start.elapsed().as_micros() as u64);
    }
  }

  /// Set link stage time from start instant
  #[inline]
  pub fn set_link_stage_time(&self, start: Option<std::time::Instant>) {
    if let (Some(collector), Some(start)) = (&self.hook_timing_collector, start) {
      #[expect(clippy::cast_possible_truncation)]
      collector.set_link_stage_micros(start.elapsed().as_micros() as u64);
    }
  }

  /// Get plugin timings summary if timing collection is enabled and plugins are taking significant time.
  /// Returns a list of (plugin_name, percentage) pairs for plugins at or above average time.
  /// Only plugins with total duration >= 1 second are included.
  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
  pub fn get_plugin_timings_info(&self) -> Option<Vec<rolldown_error::PluginTimingInfo>> {
    const MAX_PLUGINS: usize = 5;
    const ONE_SECOND_MICROS: u64 = 1_000_000;
    let collector = self.hook_timing_collector.as_ref()?;
    if !collector.plugins_are_slow() {
      return None;
    }
    let summary = collector.get_summary();
    let total_micros: u64 = summary.iter().map(|s| s.total_duration_micros).sum();
    if total_micros == 0 {
      return None;
    }
    let threshold = (total_micros / summary.len() as u64).max(ONE_SECOND_MICROS);
    let result = summary
      .iter()
      .filter(|s| s.total_duration_micros >= threshold)
      .take(MAX_PLUGINS)
      .map(|s| rolldown_error::PluginTimingInfo {
        name: s.plugin_name.to_string(),
        percent: (s.total_duration_micros as f64 / total_micros as f64 * 100.0).round() as u8,
      })
      .collect::<Vec<_>>();
    if result.is_empty() { None } else { Some(result) }
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
