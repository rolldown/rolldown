use std::sync::Arc;

use napi::tokio::sync::Mutex;
use napi_derive::napi;
use rolldown::Bundler;
use rolldown_error::BuildDiagnostic;
use rolldown_watcher::WatchEvent;

use super::binding_outputs::to_binding_error;
use super::binding_watcher_event::BindingWatcherChangeData;
use super::error::BindingError;
use crate::binding_watcher_bundler::BindingWatcherBundler;

enum NewWatcherEventInner {
  BundleEvent(WatchEvent),
  Change { path: String, kind: String },
  Restart,
  Close,
}

#[napi]
pub struct BindingNewWatcherEvent {
  inner: NewWatcherEventInner,
}

impl BindingNewWatcherEvent {
  pub fn from_watch_event(event: WatchEvent) -> Self {
    Self { inner: NewWatcherEventInner::BundleEvent(event) }
  }

  pub fn from_change(path: String, kind: String) -> Self {
    Self { inner: NewWatcherEventInner::Change { path, kind } }
  }

  pub fn from_restart() -> Self {
    Self { inner: NewWatcherEventInner::Restart }
  }

  pub fn from_close() -> Self {
    Self { inner: NewWatcherEventInner::Close }
  }
}

#[napi]
impl BindingNewWatcherEvent {
  #[napi]
  pub fn event_kind(&self) -> &str {
    match &self.inner {
      NewWatcherEventInner::BundleEvent(_) => "event",
      NewWatcherEventInner::Change { .. } => "change",
      NewWatcherEventInner::Restart => "restart",
      NewWatcherEventInner::Close => "close",
    }
  }

  #[napi]
  pub fn bundle_event_kind(&self) -> &str {
    match &self.inner {
      NewWatcherEventInner::BundleEvent(event) => event.as_str(),
      _ => unreachable!("Expected BundleEvent"),
    }
  }

  #[napi]
  pub fn bundle_end_data(&self) -> BindingNewBundleEndEventData {
    match &self.inner {
      NewWatcherEventInner::BundleEvent(WatchEvent::TaskEnd(data)) => {
        BindingNewBundleEndEventData {
          output: data.output.clone(),
          duration: data.duration,
          result: Arc::clone(&data.bundler),
        }
      }
      _ => unreachable!("Expected BundleEvent::TaskEnd"),
    }
  }

  #[napi]
  pub fn bundle_error_data(&self) -> BindingNewBundleErrorEventData {
    match &self.inner {
      NewWatcherEventInner::BundleEvent(WatchEvent::Error(data)) => {
        let errors = data
          .errors
          .iter()
          .map(|msg| {
            let diagnostic = BuildDiagnostic::unhandleable_error(anyhow::anyhow!("{msg}"));
            to_binding_error(&diagnostic, data.cwd.clone())
          })
          .collect();
        BindingNewBundleErrorEventData { error: errors, result: Arc::clone(&data.bundler) }
      }
      _ => unreachable!("Expected BundleEvent::Error"),
    }
  }

  #[napi]
  pub fn watch_change_data(&self) -> BindingWatcherChangeData {
    match &self.inner {
      NewWatcherEventInner::Change { path, kind } => {
        BindingWatcherChangeData { path: path.clone(), kind: kind.clone() }
      }
      _ => unreachable!("Expected Change event"),
    }
  }
}

#[napi]
pub struct BindingNewBundleEndEventData {
  pub output: String,
  pub duration: u32,
  result: Arc<Mutex<Bundler>>,
}

#[napi]
impl BindingNewBundleEndEventData {
  #[napi(getter)]
  pub fn result(&self) -> BindingWatcherBundler {
    BindingWatcherBundler::new(Arc::clone(&self.result))
  }
}

#[napi]
pub struct BindingNewBundleErrorEventData {
  error: Vec<BindingError>,
  result: Arc<Mutex<Bundler>>,
}

#[napi]
impl BindingNewBundleErrorEventData {
  #[napi(getter)]
  pub fn result(&self) -> BindingWatcherBundler {
    BindingWatcherBundler::new(Arc::clone(&self.result))
  }

  #[napi(getter)]
  pub fn error(&mut self) -> Vec<BindingError> {
    std::mem::take(&mut self.error)
  }
}
