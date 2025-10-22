pub mod build_driver;
pub mod build_driver_service;
pub mod build_state_machine;
pub mod building_task;
pub mod dev_context;
pub mod dev_engine;
pub mod types;
pub mod watcher_event_handler;

use std::sync::Arc;

pub use rolldown_dev_common::types::{
  BundleOutput, DevOptions, DevWatchOptions, NormalizedDevOptions, OnHmrUpdatesCallback,
  OnOutputCallback, RebuildStrategy, SharedNormalizedDevOptions, normalize_dev_options,
};
use rolldown_utils::dashmap::FxDashMap;

use crate::dev::types::client_session::ClientSession;

pub type SharedClients = Arc<FxDashMap<String, ClientSession>>;
