mod bundle_coordinator;
mod bundling_task;
mod dev_context;
mod dev_engine;
mod type_aliases;
mod types;
mod watcher_event_handler;

use std::sync::Arc;

use rolldown_utils::dashmap::FxDashMap;
pub use {
  crate::{dev_context::BundlingFuture, dev_engine::DevEngine},
  rolldown_dev_common::types::{
    BundleOutput, DevOptions, DevWatchOptions, NormalizedDevOptions, OnHmrUpdatesCallback,
    OnOutputCallback, RebuildStrategy, SharedNormalizedDevOptions, normalize_dev_options,
  },
};

use crate::types::client_session::ClientSession;

pub type SharedClients = Arc<FxDashMap<String, ClientSession>>;
