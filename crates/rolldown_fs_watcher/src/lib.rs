//! Rolldown's file watcher on top of notify.

mod config;
mod event;
mod filter;
mod notify;
mod watcher;

pub use config::FsWatcherConfig;
pub use event::{FsEvent, FsEventHandler};
pub use watcher::FsWatcher;
