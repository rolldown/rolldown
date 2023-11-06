use rolldown_fs::FileSystemExt;

use crate::{bundler::options::normalized_input_options::NormalizedInputOptions, SharedResolver};

use super::Msg;

/// Used to store common data shared between all tasks.
pub struct ModuleTaskContext<'task, T: FileSystemExt + Default> {
  pub input_options: &'task NormalizedInputOptions,
  pub tx: &'task tokio::sync::mpsc::UnboundedSender<Msg>,
  pub resolver: &'task SharedResolver<T>,
  pub fs: &'task dyn FileSystemExt,
}

impl<'task, T: FileSystemExt + Default> ModuleTaskContext<'task, T> {
  pub unsafe fn assume_static(&self) -> &'static ModuleTaskContext<'static, T> {
    std::mem::transmute(self)
  }
}
