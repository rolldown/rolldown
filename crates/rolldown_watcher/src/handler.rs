use crate::event::WatchEvent;
use rolldown_common::WatcherChangeKind;

/// Handler for watcher events. All methods are async and awaited by the coordinator,
/// providing blocking semantics matching Rollup's behavior.
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
