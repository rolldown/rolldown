use crate::{DynFsWatcher, FsWatcher};

pub trait FsWatcherExt {
  fn into_dyn_fs_watcher(self) -> DynFsWatcher;
}

impl<T> FsWatcherExt for T
where
  T: FsWatcher + Send + 'static,
{
  fn into_dyn_fs_watcher(self) -> DynFsWatcher {
    Box::new(self)
  }
}
