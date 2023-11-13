use rolldown_fs::FileSystem;

use crate::{
  bundler::{options::input_options::InputOptions, plugin_driver::SharedPluginDriver},
  SharedResolver,
};

use super::Msg;

/// Used to store common data shared between all tasks.
pub struct ModuleTaskContext<'task, T: FileSystem + Default> {
  pub input_options: &'task InputOptions,
  pub tx: &'task tokio::sync::mpsc::UnboundedSender<Msg>,
  pub resolver: &'task SharedResolver<T>,
  pub fs: &'task dyn FileSystem,
  pub plugin_driver: &'task SharedPluginDriver,
}

impl<'task, T: FileSystem + Default> ModuleTaskContext<'task, T> {
  pub unsafe fn assume_static(&self) -> &'static ModuleTaskContext<'static, T> {
    std::mem::transmute(self)
  }
}
