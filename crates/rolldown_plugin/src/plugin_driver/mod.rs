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
use rolldown_utils::{dashmap::FxDashSet, indexmap::FxIndexMap};
use sugar_path::SugarPath;
use tokio::sync::broadcast;

use crate::{
  __inner::SharedPluginable,
  PluginContext,
  plugin_driver::hook_orders::PluginHookOrders,
  type_aliases::{IndexPluginContext, IndexPluginable},
  types::{
    hook_kind::{HookKind, TimingSection},
    hook_timing::HookTimingCollector,
  },
};

pub type SharedPluginDriver = Arc<PluginDriver>;

/// `micros` as a whole-percent share of the build. Clamped because a hook can overlap
/// a section it is not part of — `pluginContext.load()` inside `buildStart` pulls
/// module loading into the serially-measured span — which can push the total slightly
/// past the build's wall clock.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn percent_of(micros: u64, total_build_micros: u64) -> u8 {
  ((micros as f64 / total_build_micros as f64 * 100.0).round() as u64).min(100) as u8
}

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

  /// Record the elapsed time of a plugin hook call if timing collection is enabled.
  #[inline]
  pub fn record_timing(
    &self,
    plugin_idx: PluginIdx,
    hook: HookKind,
    start: Option<std::time::Instant>,
  ) {
    self.record_hook_timing(Some(plugin_idx), hook, start);
  }

  /// Record the elapsed time of a user callback the Rust core invokes directly rather
  /// than through a plugin — notably the `output.codeSplitting` / `advancedChunks`
  /// `groups[].name` chunk classifier, whose cost is otherwise invisible to
  /// `[PLUGIN_TIMINGS]` even when it dominates the build.
  #[inline]
  pub fn record_core_timing(&self, hook: HookKind, start: Option<std::time::Instant>) {
    self.record_hook_timing(None, hook, start);
  }

  #[inline]
  fn record_hook_timing(
    &self,
    owner: Option<PluginIdx>,
    hook: HookKind,
    start: Option<std::time::Instant>,
  ) {
    if let (Some(collector), Some(start)) = (&self.hook_timing_collector, start) {
      #[expect(clippy::cast_possible_truncation)]
      collector.record(owner, hook, start.elapsed().as_micros() as u64);
    }
  }

  /// Record how long a section that runs hooks concurrently actually took. Hook time
  /// measured inside it is apportioned against this span, which is what makes those
  /// hooks comparable to serially-invoked ones — see [`TimingSection`].
  #[inline]
  pub fn record_section_time(&self, section: TimingSection, start: Option<std::time::Instant>) {
    if let (Some(collector), Some(start)) = (&self.hook_timing_collector, start) {
      #[expect(clippy::cast_possible_truncation)]
      collector.record_section_micros(section, start.elapsed().as_micros() as u64);
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
  ///
  /// Rows are per owner — a plugin, or the output options for callbacks the Rust core
  /// invokes directly — carrying the estimated wall-clock time attributable to them
  /// and a breakdown of which hooks it went to. Estimates are shares of real elapsed
  /// time rather than sums of measured hook durations, so a hook called serially is
  /// comparable to one called concurrently thousands of times; see [`TimingSection`].
  pub fn get_plugin_timings_info(&self) -> Option<Vec<rolldown_error::PluginTimingInfo>> {
    const MAX_ROWS: usize = 5;
    const ONE_SECOND_MICROS: u64 = 1_000_000;
    let collector = self.hook_timing_collector.as_ref()?;
    if !collector.plugins_are_slow() {
      return None;
    }
    let total_build_micros = collector.total_build_micros();
    if total_build_micros == 0 {
      return None;
    }

    let mut by_owner: FxIndexMap<ArcStr, (u64, Vec<(HookKind, u64)>)> = FxIndexMap::default();
    for estimate in collector.estimate() {
      let owner = by_owner.entry(estimate.owner).or_default();
      owner.0 += estimate.estimated_micros;
      owner.1.push((estimate.hook, estimate.estimated_micros));
    }

    // Estimates are real time, so a flat floor is a meaningful filter on its own —
    // unlike a share of the summed hook durations, which had no fixed scale.
    let mut rows = by_owner
      .into_iter()
      .filter(|(_, (total_micros, _))| *total_micros >= ONE_SECOND_MICROS)
      .collect::<Vec<_>>();
    rows.sort_by_key(|(_, (total_micros, _))| std::cmp::Reverse(*total_micros));
    rows.truncate(MAX_ROWS);

    let result = rows
      .into_iter()
      .map(|(name, (total_micros, mut hooks))| {
        hooks.sort_by_key(|(_, micros)| std::cmp::Reverse(*micros));
        rolldown_error::PluginTimingInfo {
          name: name.to_string(),
          percent: percent_of(total_micros, total_build_micros),
          estimated_ms: total_micros / 1_000,
          // A lone hook would just repeat the row it sits under.
          hooks: if hooks.len() > 1 {
            hooks
              .into_iter()
              .map(|(hook, micros)| rolldown_error::PluginHookTimingInfo {
                name: hook.label(),
                percent: percent_of(micros, total_build_micros),
                estimated_ms: micros / 1_000,
              })
              .collect()
          } else {
            Vec::new()
          },
        }
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
