//! This crate provides a customized watcher functionalities for `rolldown`.
//! Notify is a low-level library. It's not easy to use it directly.

mod event;
mod event_handler;
mod notify_watcher;
mod watcher;

pub use crate::{event::EventResult as FileChangeResult, event_handler::EventHandler};
pub use notify::RecursiveMode;
pub use {notify_watcher::NotifyWatcher, watcher::Watcher};
