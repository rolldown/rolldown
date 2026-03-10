use napi_derive::napi;
use rolldown::BundleHandle;
use rolldown_watcher::WatchEvent;

use super::binding_outputs::to_binding_error;
use super::error::BindingError;
use crate::binding_watcher_bundler::BindingWatcherBundler;

enum WatcherEventInner {
  /// Bundle event (on_event): START, BUNDLE_START, BUNDLE_END, END, ERROR
  BundleEvent(WatchEvent),
  /// File change event (on_change)
  Change { path: String, kind: String },
  /// Restart event (on_restart)
  Restart,
  /// Close event (on_close)
  Close,
}

#[napi]
pub struct BindingWatcherEvent {
  inner: WatcherEventInner,
}

impl BindingWatcherEvent {
  pub fn from_watch_event(event: WatchEvent) -> Self {
    Self { inner: WatcherEventInner::BundleEvent(event) }
  }

  pub fn from_change(path: String, kind: String) -> Self {
    Self { inner: WatcherEventInner::Change { path, kind } }
  }

  pub fn from_restart() -> Self {
    Self { inner: WatcherEventInner::Restart }
  }

  pub fn from_close() -> Self {
    Self { inner: WatcherEventInner::Close }
  }
}

#[napi]
impl BindingWatcherEvent {
  #[napi]
  pub fn event_kind(&self) -> &str {
    match &self.inner {
      WatcherEventInner::BundleEvent(_) => "event",
      WatcherEventInner::Change { .. } => "change",
      WatcherEventInner::Restart => "restart",
      WatcherEventInner::Close => "close",
    }
  }

  #[napi]
  pub fn bundle_event_kind(&self) -> &str {
    match &self.inner {
      WatcherEventInner::BundleEvent(event) => event.as_str(),
      _ => unreachable!("Expected BundleEvent"),
    }
  }

  #[napi]
  pub fn bundle_end_data(&self) -> BindingBundleEndEventData {
    match &self.inner {
      WatcherEventInner::BundleEvent(WatchEvent::BundleEnd(data)) => BindingBundleEndEventData {
        output: data.output.clone(),
        duration: data.duration,
        result: data.bundle_handle.clone(),
      },
      _ => unreachable!("Expected BundleEvent::BundleEnd"),
    }
  }

  #[napi]
  pub fn bundle_error_data(&self) -> BindingBundleErrorEventData {
    match &self.inner {
      WatcherEventInner::BundleEvent(WatchEvent::Error(data)) => BindingBundleErrorEventData {
        error: data
          .diagnostics
          .iter()
          .map(|diagnostic| to_binding_error(diagnostic, data.cwd.clone()))
          .collect(),
        result: data.bundle_handle.clone(),
      },
      _ => unreachable!("Expected BundleEvent::Error"),
    }
  }

  #[napi]
  pub fn watch_change_data(&self) -> BindingWatcherChangeData {
    match &self.inner {
      WatcherEventInner::Change { path, kind } => {
        BindingWatcherChangeData { path: path.clone(), kind: kind.clone() }
      }
      _ => unreachable!("Expected Change event"),
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
  result: BundleHandle,
}

#[napi]
impl BindingBundleEndEventData {
  #[napi(getter)]
  pub fn result(&self) -> BindingWatcherBundler {
    BindingWatcherBundler::new(self.result.clone())
  }
}

#[napi]
pub struct BindingBundleErrorEventData {
  error: Vec<BindingError>,
  result: BundleHandle,
}

#[napi]
impl BindingBundleErrorEventData {
  #[napi(getter)]
  pub fn result(&self) -> BindingWatcherBundler {
    BindingWatcherBundler::new(self.result.clone())
  }

  #[napi(getter)]
  pub fn error(&mut self) -> Vec<BindingError> {
    std::mem::take(&mut self.error)
  }
}
