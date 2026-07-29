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
  ModuleId, ModuleIdx, ModuleInfo, ModuleLoaderMsg, PluginIdx, SharedFileEmitter,
  SharedModuleInfoDashMap,
};
use rolldown_utils::dashmap::FxDashSet;
use sugar_path::SugarPath;
use tokio::sync::broadcast;

use crate::{
  __inner::SharedPluginable,
  PluginContext,
  plugin_driver::hook_orders::PluginHookOrders,
  type_aliases::{IndexPluginContext, IndexPluginable},
  types::{
    build_timings::BuildTimings,
    hook_timing::{HookTimingCollector, PluginTimingSummary},
  },
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
  pub transform_dependencies: Arc<DashMap<ModuleIdx, Arc<FxDashSet<ArcStr>>>>,
  context_load_completion_manager: ContextLoadCompletionManager,
  pub(crate) tx: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<ModuleLoaderMsg>>>>,
  /// Timing collector for plugin hooks (None if plugin timing is disabled)
  pub hook_timing_collector: Option<Arc<HookTimingCollector>>,
  /// Wall clocks for the build as a whole, as opposed to what any one plugin cost.
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
    if let Some(collector) = &self.hook_timing_collector {
      collector.clear();
    }
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

  pub fn add_transform_dependency(&self, module_idx: ModuleIdx, dependency: &str) {
    let dependency = ArcStr::from(dependency.to_slash());

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

  /// Record the elapsed time for the `output.codeSplitting` / `advancedChunks`
  /// `groups[].name` chunk-name classifier (a user JS callback invoked directly from the
  /// Rust core, not via a plugin hook) if timing collection is enabled.
  #[inline]
  pub fn record_code_splitting_name_timing(&self, start: Option<std::time::Instant>) {
    if let (Some(collector), Some(start)) = (&self.hook_timing_collector, start) {
      #[expect(clippy::cast_possible_truncation)]
      collector.record_code_splitting_name(start.elapsed().as_micros() as u64);
    }
  }

  /// Set total build time from start instant
  #[inline]
  pub fn set_total_build_time(&self, start: Option<std::time::Instant>) {
    if let Some(start) = start {
      self.build_timings.set_total(start.elapsed());
    }
  }

  /// Set link stage time from start instant
  #[inline]
  pub fn set_link_stage_time(&self, start: Option<std::time::Instant>) {
    if let Some(start) = start {
      self.build_timings.set_link_stage(start.elapsed());
    }
  }

  /// What each plugin, and each core-invoked output callback, cost this build.
  ///
  /// Raw and unaggregated: a build may produce several outputs, each with its own driver,
  /// and the report merges them — see `plugin_timings_info`.
  pub fn plugin_timing_summaries(&self) -> Vec<PluginTimingSummary> {
    let Some(collector) = self.hook_timing_collector.as_ref() else {
      return Vec::new();
    };
    let mut summaries = collector.get_summary();
    summaries.extend(collector.get_output_callback_summary());
    summaries
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
