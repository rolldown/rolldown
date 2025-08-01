use dashmap::DashMap;
use oxc::span::CompactStr;
use rustc_hash::FxHashSet;

pub static OPENED_FILE_HANDLES: std::sync::LazyLock<DashMap<CompactStr, std::fs::File>> =
  std::sync::LazyLock::new(DashMap::new);

pub static OPENED_FILES_BY_SESSION: std::sync::LazyLock<DashMap<String, FxHashSet<CompactStr>>> =
  std::sync::LazyLock::new(DashMap::new);

pub static EXIST_HASH_BY_SESSION: std::sync::LazyLock<DashMap<String, FxHashSet<String>>> =
  std::sync::LazyLock::new(DashMap::new);

pub static DEFAULT_SESSION_ID: &str = "unknown-session";
