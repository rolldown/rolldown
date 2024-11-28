use std::sync::Arc;

use napi::{tokio, Env};
use napi_derive::napi;

use crate::utils::handle_result;

use super::{binding_outputs::to_js_diagnostic, js_callback::MaybeAsyncJsCallback};
use crate::types::js_callback::MaybeAsyncJsCallbackExt;

#[napi]
pub struct BindingWatcher {
  inner: Arc<rolldown::Watcher>,
}

#[napi]
impl BindingWatcher {
  pub fn new(inner: Arc<rolldown::Watcher>) -> Self {
    Self { inner }
  }

  #[napi]
  pub async fn close(&self) -> napi::Result<()> {
    handle_result(self.inner.close().await)
  }

  #[napi(ts_args_type = "listener: (data: BindingWatcherEvent) => void")]
  pub async fn start(
    &self,
    listener: MaybeAsyncJsCallback<BindingWatcherEvent, ()>,
  ) -> napi::Result<()> {
    let rx = Arc::clone(&self.inner.emitter.rx);
    let future = async move {
      let mut run = true;
      let rx = rx.lock().await;
      while run {
        match rx.recv() {
          Ok(event) => {
            if let rolldown_common::WatcherEvent::Close = &event {
              run = false;
            }
            if let Err(e) = listener.await_call(BindingWatcherEvent::new(event)).await {
              eprintln!("watcher listener error: {e:?}");
            }
          }
          Err(e) => {
            eprintln!("watcher receiver error: {e:?}");
          }
        }
      }
    };
    #[cfg(target_family = "wasm")]
    {
      let handle = tokio::runtime::Handle::current();
      // could not block_on/spawn the main thread in WASI
      std::thread::spawn(move || {
        handle.spawn(future);
      });
    }
    #[cfg(not(target_family = "wasm"))]
    tokio::spawn(future);

    self.inner.start().await;
    Ok(())
  }
}

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
  pub fn errors(&mut self, env: Env) -> napi::Result<Vec<napi::JsUnknown>> {
    if let rolldown_common::WatcherEvent::Event(rolldown_common::BundleEvent::Error(
      rolldown_common::OutputsDiagnostics { diagnostics, cwd },
    )) = &mut self.inner
    {
<<<<<<< HEAD
      diagnostics.iter().map(|diagnostic| to_js_diagnostic(diagnostic, cwd.clone(), env)).collect()
=======
      std::mem::take(diagnostics)
        .into_iter()
        .map(|diagnostic| to_js_diagnostic(&diagnostic, cwd.clone(), env))
        .collect()
>>>>>>> 18ed45cbf (refactor: using reference at generate diagnostic)
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
