use std::sync::Arc;

use napi::tokio::sync::Mutex;
use napi_derive::napi;

use super::binding_outputs::{BindingError, to_js_diagnostic};
use rolldown::{BundleEvent, Bundler, WatcherEvent};

#[napi]
pub struct BindingWatcherEvent {
  inner: WatcherEvent,
}

#[napi]
impl BindingWatcherEvent {
  pub fn new(inner: WatcherEvent) -> Self {
    Self { inner }
  }

  #[napi]
  pub fn event_kind(&self) -> String {
    self.inner.to_string()
  }

  #[napi]
  pub fn watch_change_data(&self) -> BindingWatcherChangeData {
    match &self.inner {
      WatcherEvent::Change(data) => {
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
      WatcherEvent::Event(BundleEvent::BundleEnd(data)) => BindingBundleEndEventData {
        output: data.output.to_string(),
        duration: data.duration,
        result: Arc::clone(&data.result),
      },
      _ => {
        unreachable!("Expected WatcherEvent::Event(BundleEventKind::BundleEnd)")
      }
    }
  }

  #[napi]
  pub fn bundle_event_kind(&self) -> String {
    match &self.inner {
      WatcherEvent::Event(kind) => kind.to_string(),
      _ => {
        unreachable!("Expected WatcherEvent::Event")
      }
    }
  }

  #[napi]
  pub fn bundle_error_data(&self) -> BindingBundleErrorEventData {
    match &self.inner {
      WatcherEvent::Event(BundleEvent::Error(data)) => BindingBundleErrorEventData {
        error: data
          .error
          .diagnostics
          .iter()
          .map(|diagnostic| to_js_diagnostic(diagnostic, data.error.cwd.clone()))
          .collect(),
        result: Arc::clone(&data.result),
      },
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
  result: Arc<Mutex<Bundler>>,
}

#[napi]
impl BindingBundleEndEventData {
  #[napi(getter)]
  pub fn result(&self) -> crate::binding_bundler_impl::BindingBundlerImpl {
    crate::binding_bundler_impl::BindingBundlerImpl::new_with_bundler(Arc::clone(&self.result))
  }
}

#[napi]
pub struct BindingBundleErrorEventData {
  error: Vec<napi::Either<napi::JsError, BindingError>>,
  result: Arc<Mutex<Bundler>>,
}

#[napi]
impl BindingBundleErrorEventData {
  #[napi(getter)]
  pub fn result(&self) -> crate::binding_bundler_impl::BindingBundlerImpl {
    crate::binding_bundler_impl::BindingBundlerImpl::new_with_bundler(Arc::clone(&self.result))
  }

  #[napi(getter)]
  pub fn error(&mut self) -> Vec<napi::Either<napi::JsError, BindingError>> {
    std::mem::take(&mut self.error)
  }
}
