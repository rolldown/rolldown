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
use event_listener::Event;
use rolldown_common::{
  ModuleId, ModuleIdx, ModuleInfo, ModuleLoaderMsg, PluginIdx, SharedFileEmitter,
  SharedModuleInfoDashMap, WatchPath,
};
use rolldown_devtools::trace_action_enabled;
use rolldown_utils::dashmap::FxDashSet;

use crate::{
  __inner::SharedPluginable,
  HookFilterArgs, PluginContext, Pluginable,
  plugin_driver::hook_orders::PluginHookOrders,
  type_aliases::{IndexPluginContext, IndexPluginable},
  types::build_timings::BuildTimings,
};

pub type SharedPluginDriver = Arc<PluginDriver>;

/// `PluginContext::{info,warn,debug}` are sync but the user's `onLog` callback
/// is async, so each call spawns a detached task. The join handles are kept here
/// so the build can await them at a barrier and report errors the callback produced
pub type SharedLogHookTasks =
  Arc<Mutex<Vec<rolldown_utils::futures::JoinHandle<anyhow::Result<()>>>>>;

/// In devtools mode, the driver skips a call that the plugin's own hook filter rejects before it
/// logs the call, so the hook log holds only calls that ran. The plugin returns `None` for that
/// call anyway. See internal-docs/devtools/implementation.md.
#[inline]
fn skipped_by_hook_filter(
  plugin: &Pluginable,
  ctx: &PluginContext,
  args: HookFilterArgs<'_>,
) -> bool {
  trace_action_enabled!() && !plugin.call_hook_filter_matches(ctx, args)
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
  pub transform_dependencies: Arc<DashMap<ModuleIdx, Arc<FxDashSet<WatchPath>>>>,
  context_load_completion_manager: ContextLoadCompletionManager,
  pub(crate) tx: Arc<Mutex<Option<futures::channel::mpsc::UnboundedSender<ModuleLoaderMsg>>>>,
  /// Shared, so the binding can keep the clocks of every build until the bundler closes
  /// without keeping the driver of every build.
  pub build_timings: Arc<BuildTimings>,
  /// Pending `onLog` callback tasks spawned by `PluginContext::{info,warn,debug}`.
  pub(crate) log_hook_tasks: SharedLogHookTasks,
}

impl PluginDriver {
  pub fn clear(&self) {
    self.watch_files.clear();
    self.module_infos.clear();
    // Note: transform_dependencies is NOT cleared here - it's preserved across incremental builds
    // by BundleFactory which manages its lifecycle (reset on full builds only)
    self.context_load_completion_manager.clear();
    self.file_emitter.clear();
    // A task left over from the previous build must not fail the next one.
    self.log_hook_tasks.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
  }

  /// Await all pending `onLog` callback tasks and return the errors they produced.
  /// Drains the collector, so it is safe to call more than once
  pub async fn await_log_hook_tasks(&self) -> Vec<anyhow::Error> {
    let handles = {
      let mut guard = self.log_hook_tasks.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      std::mem::take(&mut *guard)
    };
    let mut errors = Vec::new();
    for handle in handles {
      match handle.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => errors.push(err),
        Err(join_err) => errors.push(anyhow::anyhow!("`onLog` hook task failed: {join_err}")),
      }
    }
    errors
  }

  pub fn set_module_info(&self, module_id: &ModuleId, module_info: Arc<ModuleInfo>) {
    self.module_infos.insert(module_id.as_arc_str().into(), module_info);
  }

  pub fn set_context_load_modules_tx(
    &self,
    tx: Option<futures::channel::mpsc::UnboundedSender<ModuleLoaderMsg>>,
  ) -> anyhow::Result<()> {
    *self.tx.lock().ok().context("Failed to acquire PluginDriver tx lock")? = tx;
    Ok(())
  }

  pub fn mark_context_load_progress(&self, module_id: ModuleId, progress: ContextLoadProgress) {
    self.context_load_completion_manager.advance(module_id, progress);
  }

  pub fn invalidate_context_load_module(&self, module_id: &ModuleId) {
    self.context_load_completion_manager.invalidate(module_id);
  }

  /// Without `resolve_dependencies`, this returns once the module is parsed, as in Rollup.
  /// Waiting for the imports too would deadlock a `resolveId` hook that awaits `this.load`
  /// in an import cycle.
  pub async fn wait_for_module_load_completion(&self, specifier: &str, resolve_dependencies: bool) {
    let progress = if resolve_dependencies {
      ContextLoadProgress::Completed
    } else {
      ContextLoadProgress::Parsed
    };
    self.context_load_completion_manager.wait_for(specifier.into(), progress).await;
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContextLoadProgress {
  /// The module is loaded, transformed and parsed, and its module info has the code,
  /// but its imports are not resolved yet.
  Parsed,
  /// The imports are resolved and `moduleParsed` has run. A failed module task, or an id
  /// that no module task loads (e.g. an external module), is completed too.
  Completed,
}

struct ProgressWatchState {
  progress: Option<ContextLoadProgress>,
  closed: bool,
}

struct ProgressWatch {
  state: std::sync::Mutex<ProgressWatchState>,
  event: Event,
}

impl ProgressWatch {
  fn new(initial: Option<ContextLoadProgress>) -> Arc<Self> {
    Arc::new(Self {
      state: std::sync::Mutex::new(ProgressWatchState { progress: initial, closed: false }),
      event: Event::new(),
    })
  }
}

struct ProgressSender(Arc<ProgressWatch>);

impl ProgressSender {
  fn advance(&self, progress: ContextLoadProgress) {
    let changed = {
      let mut guard = self.0.state.lock().expect("progress watch lock poisoned");
      if guard.progress.is_some_and(|current| current >= progress) {
        false
      } else {
        guard.progress = Some(progress);
        true
      }
    };
    if changed {
      self.0.event.notify(usize::MAX);
    }
  }
}

impl Drop for ProgressSender {
  fn drop(&mut self) {
    {
      let mut guard = self.0.state.lock().expect("progress watch lock poisoned");
      guard.closed = true;
    }
    self.0.event.notify(usize::MAX);
  }
}

enum ContextLoadState {
  Reached(ContextLoadProgress),
  Waited(ProgressSender),
}

#[derive(Default)]
struct ContextLoadCompletionManager {
  states: DashMap<ModuleId, ContextLoadState>,
}

impl ContextLoadCompletionManager {
  pub async fn wait_for(&self, module_id: ModuleId, progress: ContextLoadProgress) {
    let watch = match self.states.entry(module_id) {
      dashmap::Entry::Vacant(guard) => {
        let watch = ProgressWatch::new(None);
        guard.insert(ContextLoadState::Waited(ProgressSender(Arc::clone(&watch))));
        watch
      }
      dashmap::Entry::Occupied(mut guard) => match guard.get() {
        ContextLoadState::Reached(reached) if *reached >= progress => return,
        ContextLoadState::Reached(reached) => {
          let watch = ProgressWatch::new(Some(*reached));
          guard.insert(ContextLoadState::Waited(ProgressSender(Arc::clone(&watch))));
          watch
        }
        ContextLoadState::Waited(sender) => Arc::clone(&sender.0),
      },
    };
    loop {
      let listener = {
        let guard = watch.state.lock().expect("progress watch lock poisoned");
        if guard.progress.is_some_and(|current| current >= progress) {
          return;
        }
        if guard.closed {
          // This happens when `.invalidate` is called before the module completes, which is not expected
          debug_assert!(false, "The sender was dropped while waiting for module load completion");
          tracing::warn!("The sender was dropped while waiting for module load completion");
          return;
        }
        watch.event.listen()
      };
      listener.await;
    }
  }

  pub fn advance(&self, module_id: ModuleId, progress: ContextLoadProgress) {
    match self.states.entry(module_id) {
      dashmap::Entry::Vacant(guard) => {
        guard.insert(ContextLoadState::Reached(progress));
      }
      dashmap::Entry::Occupied(mut guard) => match guard.get_mut() {
        ContextLoadState::Reached(reached) => *reached = (*reached).max(progress),
        ContextLoadState::Waited(sender) => sender.advance(progress),
      },
    }
  }

  pub fn invalidate(&self, module_id: &ModuleId) {
    self.states.remove(module_id);
  }

  pub fn clear(&self) {
    self.states.clear();
  }
}
