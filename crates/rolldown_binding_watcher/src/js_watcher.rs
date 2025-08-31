use crate::js_watcher_options::JsWatcherOptions;
use napi::{Env, threadsafe_function::ThreadsafeFunctionCallMode};
use napi_derive::napi;
use rolldown_error::BuildResult;
use rolldown_watcher::{RecursiveMode, Watcher, WatcherConfig};

#[napi]
pub struct JsWatcher {
  options: JsWatcherOptions,
}

#[napi]
impl JsWatcher {
  #[napi(constructor)]
  pub fn new(_env: Env, options: JsWatcherOptions) -> napi::Result<Self> {
    Ok(Self { options })
  }
}

impl Watcher for JsWatcher {
  fn new<F: rolldown_watcher::EventHandler>(_event_handler: F) -> BuildResult<Self>
  where
    Self: Sized,
  {
    todo!("FIXME: JsWatcher doesn't support such constructor")
  }

  fn with_config<F: rolldown_watcher::EventHandler>(_event_handler: F, _config: WatcherConfig) -> BuildResult<Self>
  where
    Self: Sized,
  {
    todo!("FIXME: JsWatcher doesn't support such constructor")
  }

  fn watch(&mut self, path: &std::path::Path, _recursive_mode: RecursiveMode) -> BuildResult<()> {
    self
      .options
      .watch
      .call(Ok(path.to_string_lossy().to_string()), ThreadsafeFunctionCallMode::Blocking);
    Ok(())
  }

  fn unwatch(&mut self, path: &std::path::Path) -> BuildResult<()> {
    self
      .options
      .unwatch
      .call(Ok(path.to_string_lossy().to_string()), ThreadsafeFunctionCallMode::Blocking);

    Ok(())
  }
}
