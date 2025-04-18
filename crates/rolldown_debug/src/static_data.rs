use std::sync::Arc;

use dashmap::DashMap;
use rustc_hash::FxHashSet;

pub static OPENED_FILE_HANDLES: std::sync::LazyLock<DashMap<Arc<str>, std::fs::File>> =
  std::sync::LazyLock::new(DashMap::new);

pub static OPENED_FILES_BY_SESSION: std::sync::LazyLock<DashMap<String, FxHashSet<Arc<str>>>> =
  std::sync::LazyLock::new(DashMap::new);

pub static DEFAULT_SESSION_ID: &str = "0000000000000";
