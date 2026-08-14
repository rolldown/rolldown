use crate::event::WatchEvent;
use rolldown_common::WatcherChangeKind;

/// Handler for watcher events. Methods are async and normally awaited by the coordinator,
/// providing blocking semantics matching Rollup's behavior. A close request may interrupt the
/// wait for `on_event`, `on_change`, or `on_restart` so callbacks can close their own watcher.
///
/// NAPI bindings will implement this trait in a follow-up PR.
pub trait WatcherEventHandler: Send + Sync {
  fn on_event(&self, event: WatchEvent) -> impl std::future::Future<Output = ()> + Send;

  fn on_change(
    &self,
    path: &str,
    kind: WatcherChangeKind,
  ) -> impl std::future::Future<Output = ()> + Send;

  fn on_restart(&self) -> impl std::future::Future<Output = ()> + Send;

  fn on_close(&self) -> impl std::future::Future<Output = ()> + Send;
}
