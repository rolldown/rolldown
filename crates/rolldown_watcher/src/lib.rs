//! This crate provides a customized watcher functionalities for `rolldown`.
//! Notify is a low-level library. It's not easy to use it directly.

mod notify_watcher;
mod watcher;

pub use notify_debouncer_full::{
  DebounceEventHandler as EventHandler, DebounceEventResult as FileChangeResult,
};
pub use {notify_watcher::NotifyWatcher, watcher::Watcher};
