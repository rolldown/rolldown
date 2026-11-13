use std::sync::Arc;

use oxc::transformer_plugins::ReplaceGlobalDefinesConfig;
use rolldown_common::{ModuleId, ModuleLoaderMsg};
use rolldown_error::BuildDiagnostic;
use rolldown_fs::FileSystem;
use rolldown_plugin::SharedPluginDriver;

use crate::{SharedOptions, SharedResolver, persistent_cache::ScanCacheSession};

/// Used to store common data shared between all tasks.
pub struct TaskContext<Fs: FileSystem> {
  pub options: SharedOptions,
  pub tx: tokio::sync::mpsc::UnboundedSender<ModuleLoaderMsg>,
  pub resolver: SharedResolver<Fs>,
  pub fs: Fs,
  pub plugin_driver: SharedPluginDriver,
  pub meta: TaskContextMeta,
  pub persistent_cache: Option<Arc<ScanCacheSession>>,
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
    self.ctx.plugin_driver.ensure_context_load_module_completion(self.module_id.clone());
    let diagnostic = BuildDiagnostic::unhandleable_error(anyhow::anyhow!(
      "the task that loads `{}` panicked before it sent its result",
      self.module_id.stabilize(&self.ctx.options.cwd)
    ));
    let _ = self.ctx.tx.send(ModuleLoaderMsg::BuildErrors(vec![diagnostic].into_boxed_slice()));
  }
}
