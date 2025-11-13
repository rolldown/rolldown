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
