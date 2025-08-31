//! This crate provides a customized watcher functionalities for `rolldown`.
//! Notify is a low-level library. It's not easy to use it directly.

#[cfg(not(target_family = "wasm"))]
mod debounced_poll_watcher;
#[cfg(not(target_family = "wasm"))]
pub use debounced_poll_watcher::DebouncedPollWatcher;

mod event;
mod event_handler;
mod notify_watcher;
mod utils;
mod watcher;
mod watcher_ext;

pub use crate::{event::EventResult as FileChangeResult, event_handler::EventHandler};
pub use notify::RecursiveMode;
pub use {notify_watcher::NotifyWatcher, watcher::Watcher, watcher_ext::WatcherExt};

pub type DynWatcher = Box<dyn Watcher + Send + 'static>;
