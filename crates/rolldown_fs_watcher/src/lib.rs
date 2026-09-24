//! Rolldown's file watcher on top of notify.

mod config;
mod event;
mod notify;
mod watcher;

pub use config::FsWatcherConfig;
pub use event::{FsEvent, FsEventHandler, FsEventResult};
pub use watcher::FsWatcher;
