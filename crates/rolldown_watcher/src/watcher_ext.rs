use crate::{DynWatcher, Watcher};

pub trait WatcherExt {
  fn into_dyn_watcher(self) -> DynWatcher;
}

impl<T> WatcherExt for T
where
  T: Watcher + Send + 'static,
{
  fn into_dyn_watcher(self) -> DynWatcher {
    Box::new(self)
  }
}
