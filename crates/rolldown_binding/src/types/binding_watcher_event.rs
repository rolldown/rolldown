use napi_derive::napi;

use super::binding_outputs::{BindingError, to_js_diagnostic};

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
    match &self.inner {
      rolldown_common::WatcherEvent::Change(data) => {
        BindingWatcherChangeData { path: data.path.to_string(), kind: data.kind.to_string() }
      }
      _ => {
        unreachable!("Expected WatcherEvent::Change")
      }
    }
  }

  #[napi]
  pub fn bundle_end_data(&self) -> BindingBundleEndEventData {
    match &self.inner {
      rolldown_common::WatcherEvent::Event(rolldown_common::BundleEvent::BundleEnd(data)) => {
        BindingBundleEndEventData { output: data.output.to_string(), duration: data.duration }
      }
      _ => {
        unreachable!("Expected WatcherEvent::Event(BundleEventKind::BundleEnd)")
      }
    }
  }

  #[napi]
  pub fn bundle_event_kind(&self) -> String {
    match &self.inner {
      rolldown_common::WatcherEvent::Event(kind) => kind.to_string(),
      _ => {
        unreachable!("Expected WatcherEvent::Event")
      }
    }
  }

  #[napi]
  pub fn errors(&mut self) -> Vec<napi::Either<napi::JsError, BindingError>> {
    match &mut self.inner {
      rolldown_common::WatcherEvent::Event(rolldown_common::BundleEvent::Error(
        rolldown_common::OutputsDiagnostics { diagnostics, cwd },
      )) => {
        diagnostics.iter().map(|diagnostic| to_js_diagnostic(diagnostic, cwd.clone())).collect()
      }
      _ => {
        unreachable!("Expected WatcherEvent::Event(BundleEventKind::Error)")
      }
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
