use std::sync::Arc;
use std::time::Duration;

use napi::bindgen_prelude::FnArgs;
use napi_derive::napi;
use rolldown_common::WatcherChangeKind;
use rolldown_watcher::{WatchEvent, WatcherConfig, WatcherEventHandler};

use crate::types::binding_bundler_options::BindingBundlerOptions;
use crate::types::binding_watcher_event::BindingWatcherEvent;
use crate::types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt};
use crate::utils::create_bundler_config_from_binding_options::create_bundler_config_from_binding_options;

/// Bridges watcher events from Rust to JS via a `ThreadsafeFunction`.
struct NapiWatcherEventHandler {
  listener: Arc<MaybeAsyncJsCallback<FnArgs<(BindingWatcherEvent,)>>>,
}

impl WatcherEventHandler for NapiWatcherEventHandler {
  async fn on_event(&self, event: WatchEvent) {
    let binding_event = BindingWatcherEvent::from_watch_event(event);
    if let Err(e) = self.listener.await_call(FnArgs { data: (binding_event,) }).await {
      eprintln!("watcher on_event listener error: {e:?}");
    }
  }

  async fn on_change(&self, path: &str, kind: WatcherChangeKind) {
    let binding_event = BindingWatcherEvent::from_change(path.to_string(), kind.to_string());
    if let Err(e) = self.listener.await_call(FnArgs { data: (binding_event,) }).await {
      eprintln!("watcher on_change listener error: {e:?}");
    }
  }

  async fn on_restart(&self) {
    let binding_event = BindingWatcherEvent::from_restart();
    if let Err(e) = self.listener.await_call(FnArgs { data: (binding_event,) }).await {
      eprintln!("watcher on_restart listener error: {e:?}");
    }
  }

  async fn on_close(&self) {
    let binding_event = BindingWatcherEvent::from_close();
    if let Err(e) = self.listener.await_call(FnArgs { data: (binding_event,) }).await {
      eprintln!("watcher on_close listener error: {e:?}");
    }
  }
}

enum BindingWatcherState {
  Pending { configs: Vec<rolldown::BundlerConfig>, watcher_config: WatcherConfig },
  Running(rolldown_watcher::Watcher),
  Closed,
}

#[napi]
pub struct BindingWatcher {
  state: std::sync::Mutex<BindingWatcherState>,
  /// Stored separately so `wait_for_close` can await without holding the state mutex.
  closed_notify: std::sync::Mutex<Option<Arc<napi::tokio::sync::Notify>>>,
}

#[napi]
impl BindingWatcher {
  #[napi(constructor)]
  pub fn new(options: Vec<BindingBundlerOptions>) -> napi::Result<Self> {
    let configs = options
      .into_iter()
      .map(create_bundler_config_from_binding_options)
      .collect::<Result<Vec<_>, _>>()?;

    // Forward the largest build_delay from configs to the watcher's debounce.
    // This matches the old watcher's behavior of using the largest delay.
    let build_delay =
      configs.iter().filter_map(|c| c.options.watch.as_ref().and_then(|w| w.build_delay)).max();

    // Extract use_polling / poll_interval from the first config that specifies them.
    let use_polling = configs
      .iter()
      .find_map(|c| c.options.watch.as_ref().filter(|w| w.use_polling).map(|w| w.use_polling))
      .unwrap_or(false);
    let poll_interval =
      configs.iter().find_map(|c| c.options.watch.as_ref().and_then(|w| w.poll_interval));

    let watcher_config = WatcherConfig {
      debounce: build_delay.map(|ms| Duration::from_millis(u64::from(ms))),
      use_polling,
      poll_interval,
    };

    Ok(Self {
      state: std::sync::Mutex::new(BindingWatcherState::Pending { configs, watcher_config }),
      closed_notify: std::sync::Mutex::new(None),
    })
  }

  #[tracing::instrument(level = "debug", skip_all)]
  #[napi(ts_args_type = "listener: (data: BindingWatcherEvent) => void")]
  pub async fn start(
    &self,
    listener: MaybeAsyncJsCallback<FnArgs<(BindingWatcherEvent,)>>,
  ) -> napi::Result<()> {
    let mut state = self
      .state
      .lock()
      .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("Lock poisoned: {e}")))?;

    let (configs, watcher_config) =
      match std::mem::replace(&mut *state, BindingWatcherState::Closed) {
        BindingWatcherState::Pending { configs, watcher_config } => (configs, watcher_config),
        other => {
          *state = other;
          return Err(napi::Error::new(
            napi::Status::GenericFailure,
            "Watcher is not in Pending state (already started or closed)",
          ));
        }
      };

    let handler = NapiWatcherEventHandler { listener: Arc::new(listener) };
    let watcher = match rolldown_watcher::Watcher::with_multiple_bundler_configs(
      configs,
      handler,
      &watcher_config,
    ) {
      Ok(w) => w,
      Err(errs) => {
        // Restore Pending state so the watcher can be retried.
        *state = BindingWatcherState::Pending { configs: Vec::new(), watcher_config };
        return Err(napi::Error::new(
          napi::Status::GenericFailure,
          errs.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
        ));
      }
    };

    // Store the closed_notify handle for wait_for_close()
    let mut notify = self
      .closed_notify
      .lock()
      .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("Lock poisoned: {e}")))?;
    *notify = Some(watcher.closed_notify());

    *state = BindingWatcherState::Running(watcher);
    Ok(())
  }

  /// Returns a Promise that resolves when the watcher closes.
  /// The pending Promise keeps Node.js event loop alive (replaces setInterval hack).
  #[tracing::instrument(level = "debug", skip_all)]
  #[napi]
  pub async fn wait_for_close(&self) -> napi::Result<()> {
    let notify = {
      let guard = self.closed_notify.lock().map_err(|e| {
        napi::Error::new(napi::Status::GenericFailure, format!("Lock poisoned: {e}"))
      })?;
      guard.clone()
    };

    match notify {
      Some(n) => {
        n.notified().await;
        Ok(())
      }
      None => Err(napi::Error::new(napi::Status::GenericFailure, "Watcher is not running")),
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  #[napi]
  pub async fn close(&self) -> napi::Result<()> {
    let watcher = {
      let mut state = self.state.lock().map_err(|e| {
        napi::Error::new(napi::Status::GenericFailure, format!("Lock poisoned: {e}"))
      })?;
      match std::mem::replace(&mut *state, BindingWatcherState::Closed) {
        BindingWatcherState::Running(watcher) => Some(watcher),
        other => {
          *state = other;
          None
        }
      }
    };

    if let Some(watcher) = watcher {
      watcher
        .close()
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))?;
    }

    Ok(())
  }
}
