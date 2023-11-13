use rolldown_fs::FileSystem;

use crate::{
  bundler::{options::input_options::InputOptions, plugin_driver::SharedPluginDriver},
  SharedResolver,
};

use super::Msg;

/// Used to store common data shared between all tasks.
pub struct ModuleTaskCommonData<'task, T: FileSystem + Default> {
  pub input_options: &'task InputOptions,
  pub tx: tokio::sync::mpsc::UnboundedSender<Msg>,
  pub resolver: SharedResolver<T>,
  pub fs: T,
  pub plugin_driver: SharedPluginDriver,
}

impl<'task, T: FileSystem + Default> ModuleTaskCommonData<'task, T> {
  pub unsafe fn assume_static(&self) -> &'static ModuleTaskCommonData<'static, T> {
    std::mem::transmute(self)
  }
}
