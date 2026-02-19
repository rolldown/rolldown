mod bundle_coordinator;
mod bundling_task;
mod dev_context;
mod dev_engine;
mod type_aliases;
mod types;
mod watcher_event_handler;

use std::sync::Arc;

use rustc_hash::FxHashMap;
use tokio::sync::Mutex;
pub use {
  crate::{
    dev_context::BundlingFuture,
    dev_engine::{BundleState, DevEngine},
  },
  rolldown::BundlerConfig,
  rolldown_dev_common::types::{
    BundleOutput, DevOptions, DevWatchOptions, NormalizedDevOptions, OnHmrUpdatesCallback,
    OnOutputCallback, RebuildStrategy, SharedNormalizedDevOptions, normalize_dev_options,
  },
};

use crate::types::client_session::ClientSession;

// Multiple clients are not accessed from multiple threads simultaneously
// so we use a Mutex<FxHashMap> instead of a DashMap
pub type SharedClients = Arc<Mutex<FxHashMap<String, ClientSession>>>;
