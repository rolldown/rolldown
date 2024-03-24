use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;

use crate::{options::normalized_input_options::SharedNormalizedInputOptions, SharedResolver};

use super::Msg;

/// Used to store common data shared between all tasks.
pub struct ModuleTaskCommonData {
  pub input_options: SharedNormalizedInputOptions,
  pub tx: tokio::sync::mpsc::UnboundedSender<Msg>,
  pub resolver: SharedResolver,
  pub fs: OsFileSystem,
  pub plugin_driver: SharedPluginDriver,
}

impl ModuleTaskCommonData {
  pub unsafe fn assume_static(&self) -> &'static Self {
    std::mem::transmute(self)
  }
}
