use oxc::transformer_plugins::ReplaceGlobalDefinesConfig;
use rolldown_common::ModuleLoaderMsg;
use rolldown_fs::FileSystem;
use rolldown_plugin::SharedPluginDriver;

use crate::{SharedOptions, SharedResolver};

/// Used to store common data shared between all tasks.
pub struct TaskContext<Fs: FileSystem> {
  pub options: SharedOptions,
  pub tx: crossbeam_channel::Sender<ModuleLoaderMsg>,
  pub resolver: SharedResolver<Fs>,
  pub fs: Fs,
  pub plugin_driver: SharedPluginDriver,
  pub meta: TaskContextMeta,
  pub tokio_handle: tokio::runtime::Handle,
}

pub struct TaskContextMeta {
  pub replace_global_define_config: Option<ReplaceGlobalDefinesConfig>,
}
