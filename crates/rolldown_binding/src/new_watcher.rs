use std::sync::Arc;

use napi::bindgen_prelude::FnArgs;
use napi_derive::napi;
use rolldown::BundlerConfig;
use rolldown_common::WatcherChangeKind;
use rolldown_watcher::{WatchEvent, WatcherConfig, WatcherEventHandler};

use crate::types::binding_bundler_options::BindingBundlerOptions;
use crate::types::binding_new_watcher_event::BindingNewWatcherEvent;
use crate::types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt};
use crate::utils::create_bundler_config_from_binding_options::create_bundler_config_from_binding_options;
use crate::utils::handle_result;
use crate::watcher::BindingNotifyOption;

pub(crate) struct BindingNewWatcherHandler {
  listener: Arc<MaybeAsyncJsCallback<FnArgs<(BindingNewWatcherEvent,)>>>,
}

impl WatcherEventHandler for BindingNewWatcherHandler {
  async fn on_event(&self, event: WatchEvent) {
    let binding_event = BindingNewWatcherEvent::from_watch_event(event);
    if let Err(e) = self.listener.await_call(FnArgs { data: (binding_event,) }).await {
      eprintln!("new watcher on_event listener error: {e:?}");
    }
  }

  async fn on_change(&self, path: &str, kind: WatcherChangeKind) {
    let binding_event = BindingNewWatcherEvent::from_change(path.to_string(), kind.to_string());
    if let Err(e) = self.listener.await_call(FnArgs { data: (binding_event,) }).await {
      eprintln!("new watcher on_change listener error: {e:?}");
    }
  }

  async fn on_restart(&self) {
    let binding_event = BindingNewWatcherEvent::from_restart();
    if let Err(e) = self.listener.await_call(FnArgs { data: (binding_event,) }).await {
      eprintln!("new watcher on_restart listener error: {e:?}");
    }
  }

  async fn on_close(&self) {
    let binding_event = BindingNewWatcherEvent::from_close();
    if let Err(e) = self.listener.await_call(FnArgs { data: (binding_event,) }).await {
      eprintln!("new watcher on_close listener error: {e:?}");
    }
  }
}

enum BindingNewWatcherState {
  Pending { configs: Vec<BundlerConfig>, watcher_config: WatcherConfig },
  Running(rolldown_watcher::Watcher),
  Closed,
}

#[napi]
pub struct BindingNewWatcher {
  state: std::sync::Mutex<BindingNewWatcherState>,
}

#[napi]
impl BindingNewWatcher {
  #[napi(constructor)]
  pub fn new(
    options: Vec<BindingBundlerOptions>,
    notify_option: Option<BindingNotifyOption>,
  ) -> napi::Result<Self> {
    let configs = options
      .into_iter()
      .map(create_bundler_config_from_binding_options)
      .collect::<Result<Vec<_>, _>>()?;

    let watcher_config = WatcherConfig { debounce: None, notify: notify_option.map(Into::into) };

    Ok(Self {
      state: std::sync::Mutex::new(BindingNewWatcherState::Pending { configs, watcher_config }),
    })
  }

  #[tracing::instrument(level = "debug", skip_all)]
  #[napi(ts_args_type = "listener: (data: BindingNewWatcherEvent) => void")]
  pub async fn start(
    &self,
    listener: MaybeAsyncJsCallback<FnArgs<(BindingNewWatcherEvent,)>>,
  ) -> napi::Result<()> {
    let (configs, watcher_config) = {
      let mut guard = self.state.lock().unwrap();
      match std::mem::replace(&mut *guard, BindingNewWatcherState::Closed) {
        BindingNewWatcherState::Pending { configs, watcher_config } => (configs, watcher_config),
        other => {
          // Put it back
          *guard = other;
          return Err(napi::Error::from_reason("Watcher is not in pending state"));
        }
      }
    };

    let handler = BindingNewWatcherHandler { listener: Arc::new(listener) };
    let watcher =
      rolldown_watcher::Watcher::with_multiple_bundler_configs(configs, handler, &watcher_config)
        .map_err(|errs| {
        napi::Error::new(
          napi::Status::GenericFailure,
          errs.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
        )
      })?;

    {
      let mut guard = self.state.lock().unwrap();
      *guard = BindingNewWatcherState::Running(watcher);
    }

    Ok(())
  }

  #[tracing::instrument(level = "debug", skip_all)]
  #[napi]
  pub async fn close(&self) -> napi::Result<()> {
    let watcher = {
      let mut guard = self.state.lock().unwrap();
      match std::mem::replace(&mut *guard, BindingNewWatcherState::Closed) {
        BindingNewWatcherState::Running(watcher) => watcher,
        other => {
          *guard = other;
          return Ok(());
        }
      }
    };
    handle_result(watcher.close().await)
  }
}
