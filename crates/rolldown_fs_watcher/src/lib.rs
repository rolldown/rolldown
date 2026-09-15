//! This crate provides a customized filesystem watcher functionalities for `rolldown`.
//! Notify is a low-level library. It's not easy to use it directly.

mod config;
mod event;
mod event_map;
mod notify;
mod watcher;

pub use ::notify::RecursiveMode;
pub use config::FsWatcherConfig;
pub use event::{FsEvent, FsEventHandler, FsEventResult};
pub use event_map::{FsChangeKind, map_notify_event};
pub use watcher::{FsWatcher, PathsMut};
