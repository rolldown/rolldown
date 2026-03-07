//! This crate provides a customized filesystem watcher functionalities for `rolldown`.
//! Notify is a low-level library. It's not easy to use it directly.

mod fs_event;
mod fs_event_handler;
mod fs_watcher;
mod fs_watcher_config;
mod fs_watcher_ext;
mod noop_fs_watcher;
mod paths_mut;
mod recommended_fs_watcher;
mod utils;

#[cfg(not(target_family = "wasm"))]
mod poll_fs_watcher;
#[cfg(not(target_family = "wasm"))]
pub use poll_fs_watcher::PollFsWatcher;

#[cfg(not(target_family = "wasm"))]
mod debounced_poll_fs_watcher;
#[cfg(not(target_family = "wasm"))]
pub use debounced_poll_fs_watcher::DebouncedPollFsWatcher;

#[cfg(not(target_family = "wasm"))]
mod debounced_recommended_fs_watcher;
#[cfg(not(target_family = "wasm"))]
pub use debounced_recommended_fs_watcher::DebouncedRecommendedFsWatcher;

pub use crate::{
  fs_event::{FsEvent, FsEventResult},
  fs_event_handler::FsEventHandler,
};
pub use fs_watcher::FsWatcher;
pub use fs_watcher_config::FsWatcherConfig;
pub use fs_watcher_ext::FsWatcherExt;
pub use noop_fs_watcher::NoopFsWatcher;
pub use notify::RecursiveMode;
pub use paths_mut::PathsMut;
pub use recommended_fs_watcher::RecommendedFsWatcher;

pub type DynFsWatcher = Box<dyn FsWatcher + Send + 'static>;

/// Create a filesystem watcher based on the `use_polling` and `use_debounce` fields in the config.
///
/// This is the canonical factory for non-noop watchers. Consumers that need a `NoopFsWatcher`
/// (e.g. when watching is disabled) should construct it directly.
pub fn create_fs_watcher<F: FsEventHandler>(
  event_handler: F,
  config: FsWatcherConfig,
) -> rolldown_error::BuildResult<DynFsWatcher> {
  #[cfg(not(target_family = "wasm"))]
  {
    match (config.use_polling, config.use_debounce) {
      (true, false) => Ok(PollFsWatcher::with_config(event_handler, config)?.into_dyn_fs_watcher()),
      (true, true) => {
        Ok(DebouncedPollFsWatcher::with_config(event_handler, config)?.into_dyn_fs_watcher())
      }
      (false, false) => {
        Ok(RecommendedFsWatcher::with_config(event_handler, config)?.into_dyn_fs_watcher())
      }
      (false, true) => {
        Ok(DebouncedRecommendedFsWatcher::with_config(event_handler, config)?.into_dyn_fs_watcher())
      }
    }
  }
  #[cfg(target_family = "wasm")]
  {
    Ok(RecommendedFsWatcher::with_config(event_handler, config)?.into_dyn_fs_watcher())
  }
}
