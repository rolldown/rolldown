use napi::Env;
use napi_derive::napi;

use super::binding_outputs::to_js_diagnostic;

#[napi]
pub struct BindingWatcherEvent {
  inner: rolldown_common::WatcherEvent,
}

#[napi]
impl BindingWatcherEvent {
  pub fn new(inner: rolldown_common::WatcherEvent) -> Self {
    Self { inner }
  }

  #[napi]
  pub fn event_kind(&self) -> String {
    self.inner.to_string()
  }

  #[napi]
  pub fn watch_change_data(&self) -> BindingWatcherChangeData {
    if let rolldown_common::WatcherEvent::Change(data) = &self.inner {
      BindingWatcherChangeData { path: data.path.to_string(), kind: data.kind.to_string() }
    } else {
      unreachable!("Expected WatcherEvent::Change")
    }
  }

  #[napi]
  pub fn bundle_end_data(&self) -> BindingBundleEndEventData {
    if let rolldown_common::WatcherEvent::Event(rolldown_common::BundleEvent::BundleEnd(data)) =
      &self.inner
    {
      BindingBundleEndEventData { output: data.output.to_string(), duration: data.duration }
    } else {
      unreachable!("Expected WatcherEvent::Event(BundleEventKind::BundleEnd)")
    }
  }

  #[napi]
  pub fn bundle_event_kind(&self) -> String {
    if let rolldown_common::WatcherEvent::Event(kind) = &self.inner {
      kind.to_string()
    } else {
      unreachable!("Expected WatcherEvent::Event")
    }
  }

  #[napi]
  pub fn errors(
    &mut self,
    env: Env,
  ) -> napi::Result<Vec<napi::Either<napi::JsError, napi::JsObject>>> {
    if let rolldown_common::WatcherEvent::Event(rolldown_common::BundleEvent::Error(
      rolldown_common::OutputsDiagnostics { diagnostics, cwd },
    )) = &mut self.inner
    {
      diagnostics.iter().map(|diagnostic| to_js_diagnostic(diagnostic, cwd.clone(), env)).collect()
    } else {
      unreachable!("Expected WatcherEvent::Event(BundleEventKind::Error)")
    }
  }
}

#[napi]
pub struct BindingWatcherChangeData {
  pub path: String,
  pub kind: String,
}

#[napi]
pub struct BindingBundleEndEventData {
  pub output: String,
  pub duration: u32,
}
