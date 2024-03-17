use rolldown_fs::FileSystem;
use rolldown_plugin::SharedPluginDriver;

use crate::{options::normalized_input_options::SharedNormalizedInputOptions, SharedResolver};

use super::Msg;

/// Used to store common data shared between all tasks.
pub struct ModuleTaskCommonData<T: FileSystem + Default> {
  pub input_options: SharedNormalizedInputOptions,
  pub tx: tokio::sync::mpsc::UnboundedSender<Msg>,
  pub resolver: SharedResolver<T>,
  pub fs: T,
  pub plugin_driver: SharedPluginDriver,
}

impl<T: FileSystem + Default> ModuleTaskCommonData<T> {
  pub unsafe fn assume_static(&self) -> &'static Self {
    std::mem::transmute(self)
  }
}
