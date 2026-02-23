use oxc::transformer_plugins::ReplaceGlobalDefinesConfig;
use rolldown_common::ModuleLoaderMsg;
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;

use crate::{SharedOptions, SharedResolver};

/// Used to store common data shared between all tasks.
pub struct TaskContext {
  pub options: SharedOptions,
  pub tx: tokio::sync::mpsc::Sender<ModuleLoaderMsg>,
  pub resolver: SharedResolver,
  pub fs: OsFileSystem,
  pub plugin_driver: SharedPluginDriver,
  pub meta: TaskContextMeta,
  /// Limits concurrent filesystem I/O to avoid APFS global kernel lock contention on macOS.
  /// On macOS, too many threads calling open() concurrently causes severe lock contention
  /// due to the APFS global kernel lock, degrading open() latency by 4x+.
  pub fs_semaphore: tokio::sync::Semaphore,
}

pub struct TaskContextMeta {
  pub replace_global_define_config: Option<ReplaceGlobalDefinesConfig>,
}
