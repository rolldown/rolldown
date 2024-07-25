use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;

use crate::{SharedOptions, SharedResolver};

use super::Msg;

/// Used to store common data shared between all tasks.
pub struct TaskContext {
  pub options: SharedOptions,
  pub tx: tokio::sync::mpsc::Sender<Msg>,
  pub resolver: SharedResolver,
  pub fs: OsFileSystem,
  pub plugin_driver: SharedPluginDriver,
}
