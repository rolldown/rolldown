use std::sync::Arc;

use oxc::transformer_plugins::ReplaceGlobalDefinesConfig;
use rolldown_common::{JsModuleMeta, ModuleId, ModuleInfo, ModuleLoaderMsg, ResolvedId};
use rolldown_error::BuildDiagnostic;
use rolldown_fs::FileSystem;
use rolldown_plugin::{ContextLoadProgress, SharedPluginDriver};
use rolldown_utils::dashmap::FxDashSet;

use crate::{SharedOptions, SharedResolver, persistent_cache::ScanCacheSession};

/// Used to store common data shared between all tasks.
pub struct TaskContext<Fs: FileSystem> {
  pub options: SharedOptions,
  pub tx: tokio::sync::mpsc::UnboundedSender<ModuleLoaderMsg>,
  pub resolver: SharedResolver<Fs>,
  pub fs: Fs,
  pub plugin_driver: SharedPluginDriver,
  pub meta: TaskContextMeta,
  /// Entries whose module task started before they were resolved.
  pub late_entries: FxDashSet<ModuleId>,
  pub persistent_cache: Option<Arc<ScanCacheSession>>,
}

impl<Fs: FileSystem> TaskContext<Fs> {
  pub fn set_module_info(&self, mut module_info: ModuleInfo) -> Arc<ModuleInfo> {
    // `mark_entry` waits for this entry, so it cannot run between the check and the insert.
    let entry = self.plugin_driver.module_infos.entry(module_info.id.as_arc_str().clone());
    module_info.is_entry |= self.late_entries.contains(&module_info.id);
    let module_info = Arc::new(module_info);
    entry.insert(Arc::clone(&module_info));
    module_info
  }

  /// Marks a module as an entry after its module task has started as a non-entry.
  pub fn mark_entry(&self, module_id: &ModuleId) {
    self.late_entries.insert(module_id.clone());
    if let Some(mut module_info) = self.plugin_driver.module_infos.get_mut(module_id.as_arc_str())
      && !module_info.is_entry
    {
      Arc::make_mut(module_info.value_mut()).is_entry = true;
    }
  }
}

impl<Fs: FileSystem> TaskContext<Fs> {
  /// Tells JS that `resolved_id` created a module. Call it before the `load` hook runs.
  /// `saved_own_meta` is the `meta` of the skipped hooks of a module that reuses its saved result.
  pub async fn create_js_module_options(
    &self,
    resolved_id: &ResolvedId,
    saved_own_meta: Option<&JsModuleMeta>,
  ) -> anyhow::Result<()> {
    let Some(create_js_module_options) = &self.options.create_js_module_options else {
      return Ok(());
    };
    // A live resolution has a receipt, and JS has its `meta` already.
    let saved_meta = match resolved_id.module_options_receipt {
      Some(_) => None,
      None => resolved_id.js_meta.as_ref().map(|meta| meta.to_entries_json()),
    };
    create_js_module_options
      .call(
        resolved_id.id.to_string(),
        resolved_id.module_options_receipt,
        saved_meta,
        saved_own_meta.map(JsModuleMeta::to_entries_json),
      )
      .await
  }
}

pub struct TaskContextMeta {
  pub replace_global_define_config: Option<ReplaceGlobalDefinesConfig>,
}

/// The loader waits for one message from each module task. If a task panics before it sends
/// that message, this guard sends an error in its place, so the build fails instead of hanging.
pub struct TaskMessageGuard<Fs: FileSystem> {
  ctx: Arc<TaskContext<Fs>>,
  module_id: ModuleId,
  sent: bool,
}

impl<Fs: FileSystem> TaskMessageGuard<Fs> {
  pub fn new(ctx: &Arc<TaskContext<Fs>>, module_id: ModuleId) -> Self {
    Self { ctx: Arc::clone(ctx), module_id, sent: false }
  }

  /// Call this after the task has sent its message.
  pub fn mark_sent(mut self) {
    self.sent = true;
  }
}

impl<Fs: FileSystem> Drop for TaskMessageGuard<Fs> {
  fn drop(&mut self) {
    if self.sent {
      return;
    }
    // A `this.load` of this module waits until the module is loaded. End that wait too.
    self
      .ctx
      .plugin_driver
      .mark_context_load_progress(self.module_id.clone(), ContextLoadProgress::Completed);
    let diagnostic = BuildDiagnostic::unhandleable_error(anyhow::anyhow!(
      "the task that loads `{}` panicked before it sent its result",
      self.module_id.stabilize(&self.ctx.options.cwd)
    ));
    let _ = self.ctx.tx.send(ModuleLoaderMsg::BuildErrors(vec![diagnostic].into_boxed_slice()));
  }
}
