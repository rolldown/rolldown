//! The persistent scan cache: saves the result of the scan hooks of each module to disk and
//! reuses it after a restart. See `internal-docs/persistent-cache/implementation.md`.

mod codec;
pub mod saved_result;

use std::{
  borrow::Cow,
  path::PathBuf,
  sync::{Arc, Mutex},
  time::Duration,
};

use rolldown_cache_store::{CacheStore, Key, StoreOptions};
use rolldown_common::{
  Log, LogLevel, ModuleId, ModuleType, NormalizedBundlerOptions, SharedNormalizedBundlerOptions,
};
use rolldown_fs::FileSystem;
use rolldown_utils::{
  dashmap::FxDashMap,
  pattern_filter::{StringOrRegex, StringOrRegexMatchKind, normalize_path},
};
use xxhash_rust::xxh3::Xxh3Default;

use self::saved_result::SavedResult;

/// The input of every key 1. The Rolldown version stands in for a cache version: a new version
/// can change the result of a module in ways that no other input shows.
const ROLLDOWN_VERSION: &str = env!("CARGO_PKG_VERSION");

pub type SharedPersistentScanCache = Arc<PersistentScanCache>;

/// Lives as long as the `BundleFactory`, so watch mode and the dev engine keep one store open
/// across rebuilds.
#[derive(Debug)]
pub struct PersistentScanCache {
  store: Arc<CacheStore>,
  /// Hash of the cache key, the options hash and the plugin names.
  key1_prefix: u128,
  build_dependencies: Vec<PathBuf>,
  /// Empty when the option is not set, or when `resolve.symlinks` is `false`.
  assume_unchanged: Vec<StringOrRegex>,
  cwd: String,
  pending_flush: Mutex<Option<std::thread::JoinHandle<std::io::Result<()>>>>,
  /// The hash of the build dependencies at the first scan. The options and plugins of this
  /// factory were made from them. See `start_scan`.
  first_build_dependencies_hash: std::sync::OnceLock<u128>,
}

impl PersistentScanCache {
  /// `None` when `experimental.cache` is off, or when the options do not allow the cache.
  pub fn new<'a>(
    options: &NormalizedBundlerOptions,
    options_hash: Option<u128>,
    resolve_symlinks: bool,
    plugin_names: impl Iterator<Item = Cow<'a, str>>,
  ) -> Option<SharedPersistentScanCache> {
    let cache_options = options.experimental.cache.as_ref()?;
    // With native MagicString, a `transform` sourcemap is added to the module after the scan,
    // not during the module task, so a saved result would miss it.
    if options.experimental.is_native_magic_string_enabled() && options.is_sourcemap_enabled() {
      return None;
    }
    let options_hash = options_hash?;

    let mut hasher = Xxh3Default::new();
    hash_str(&mut hasher, ROLLDOWN_VERSION);
    hash_str(&mut hasher, cache_options.cache_key.as_deref().unwrap_or_default());
    hasher.update(&[u8::from(cache_options.watch_mode.unwrap_or(false))]);
    hasher.update(&options_hash.to_le_bytes());
    for name in plugin_names {
      hash_str(&mut hasher, &name);
    }

    let cwd = &options.cwd;
    let max_age = cache_options
      .max_age
      .filter(|ms| ms.is_finite() && *ms >= 0.0)
      .map(|ms| Duration::from_secs_f64(ms / 1000.0));
    let store = CacheStore::open(StoreOptions { dir: cwd.join(&cache_options.dir), max_age });

    Some(Arc::new(Self {
      store: Arc::new(store),
      key1_prefix: hasher.digest128(),
      build_dependencies: cache_options
        .build_dependencies
        .iter()
        .flatten()
        .map(|path| cwd.join(path))
        .collect(),
      assume_unchanged: if resolve_symlinks {
        cache_options.assume_unchanged.clone().unwrap_or_default()
      } else {
        vec![]
      },
      cwd: cwd.to_string_lossy().into_owned(),
      pending_flush: Mutex::new(None),
      first_build_dependencies_hash: std::sync::OnceLock::new(),
    }))
  }

  /// Starts the cache for one scan. Only a full scan reads saved results. An incremental scan
  /// reuses the scan results in memory, which is faster, and only saves new results.
  pub async fn start_scan<Fs: FileSystem + Clone + 'static>(
    self: &Arc<Self>,
    fs: &Fs,
    read: bool,
    options: &SharedNormalizedBundlerOptions,
  ) -> Arc<ScanCacheSession> {
    self.wait_for_flush(options).await;
    if read {
      let store = Arc::clone(&self.store);
      run_blocking(move || store.refresh()).await;
    }
    let session = ScanCacheSession {
      cache: Arc::clone(self),
      key1_base: 0,
      read,
      enabled: true,
      file_states: FxDashMap::default(),
    };
    // A build dependency is always hashed, also when its path matches `assumeUnchanged`.
    let mut hasher = Xxh3Default::new();
    for path in &self.build_dependencies {
      let path = path.to_string_lossy();
      hash_str(&mut hasher, &path);
      session.file_state(fs, &path).await.hash_into(&mut hasher);
    }
    let build_dependencies_hash = hasher.digest128();
    // A build dependency changed while this bundler runs, for example the config file in watch
    // mode. The options and plugins still come from the old one, so a result would be saved
    // under a key that says the new one. The cache stays off until a restart.
    let enabled = *self.first_build_dependencies_hash.get_or_init(|| build_dependencies_hash)
      == build_dependencies_hash;
    let mut hasher = Xxh3Default::new();
    hasher.update(&self.key1_prefix.to_le_bytes());
    hasher.update(&build_dependencies_hash.to_le_bytes());
    Arc::new(ScanCacheSession { key1_base: hasher.digest128(), enabled, ..session })
  }

  /// Writes the results saved during the last scan in the background. A failed write does not
  /// fail the build. The next scan, or `wait_for_flush`, reports it as a warning.
  pub fn start_flush(&self) {
    if !self.store.has_pending() {
      return;
    }
    let store = Arc::clone(&self.store);
    let handle = std::thread::spawn(move || store.flush());
    if let Ok(mut slot) = self.pending_flush.lock() {
      // A flush that is still running stays correct: a store never changes data on disk.
      if let Some(previous) = slot.replace(handle) {
        let _ = previous.join();
      }
    }
  }

  /// Waits for the write that `start_flush` started, and prints a warning if it failed.
  pub async fn wait_for_flush(&self, options: &SharedNormalizedBundlerOptions) {
    let Some(handle) = self.pending_flush.lock().ok().and_then(|mut slot| slot.take()) else {
      return;
    };
    let result = run_blocking(move || handle.join()).await;
    let error = match result {
      Ok(Ok(())) => return,
      Ok(Err(error)) => error.to_string(),
      Err(_) => "the writer thread panicked".to_string(),
    };
    if let Some(on_log) = &options.on_log {
      let _ = on_log
        .call(
          LogLevel::Warn,
          Log {
            message: format!(
              "Could not write the persistent cache: {error}. The build is not affected, but \
               the next build may not reuse its results."
            ),
            code: Some("PERSISTENT_CACHE_WRITE_FAILED".to_string()),
            ..Default::default()
          },
        )
        .await;
    }
  }

  fn assumed_unchanged(&self, path: &str) -> bool {
    if self.assume_unchanged.is_empty() {
      return false;
    }
    let path = normalize_path(path);
    let kind = StringOrRegexMatchKind::Id(&self.cwd);
    self.assume_unchanged.iter().any(|pattern| pattern.test(&path, &kind))
  }
}

impl Drop for PersistentScanCache {
  fn drop(&mut self) {
    if let Some(handle) = self.pending_flush.get_mut().ok().and_then(Option::take) {
      let _ = handle.join();
    }
  }
}

/// The state of a file, as in Restart Validation. Rolldown reads each file once per scan.
#[derive(Debug, Clone, Copy)]
enum FileState {
  Missing,
  Hash(u128),
}

impl FileState {
  fn hash_into(self, hasher: &mut Xxh3Default) {
    match self {
      FileState::Missing => hasher.update(&[0]),
      FileState::Hash(hash) => {
        hasher.update(&[1]);
        hasher.update(&hash.to_le_bytes());
      }
    }
  }
}

/// The cache during one scan.
#[derive(Debug)]
pub struct ScanCacheSession {
  cache: SharedPersistentScanCache,
  key1_base: u128,
  read: bool,
  enabled: bool,
  file_states: FxDashMap<String, FileState>,
}

/// Key 1 of a module, and the contents of its file when Rolldown read them to compute the key.
pub struct ModuleKey {
  pub key: Key,
  pub contents: Option<Vec<u8>>,
}

impl ScanCacheSession {
  pub fn reads_saved_results(&self) -> bool {
    self.read && self.enabled
  }

  /// Key 1 of a module, or `None` when the module cannot get a saved result: its id is not the
  /// absolute path of a file that exists.
  pub async fn module_key<Fs: FileSystem + Clone + 'static>(
    &self,
    fs: &Fs,
    id: &ModuleId,
    asserted_module_type: Option<&ModuleType>,
  ) -> Option<ModuleKey> {
    if !self.enabled {
      return None;
    }
    let path = id.as_path()?;
    let raw_id = id.raw_id();
    let mut hasher = Xxh3Default::new();
    hasher.update(&self.key1_base.to_le_bytes());
    hash_str(&mut hasher, id.as_str());
    match asserted_module_type {
      None => hasher.update(&[0]),
      Some(ty) => {
        hasher.update(&[1]);
        hash_str(&mut hasher, &format!("{ty:?}"));
      }
    }
    if self.cache.assumed_unchanged(raw_id) {
      let fs = fs.clone();
      let path = path.to_path_buf();
      if !run_blocking(move || matches!(fs.metadata(&path), Ok(m) if m.is_file())).await {
        return None;
      }
      hasher.update(&[0]);
      return Some(ModuleKey { key: hasher.digest128(), contents: None });
    }
    let fs = fs.clone();
    let path = path.to_path_buf();
    let contents = run_blocking(move || fs.read(&path)).await.ok()?;
    let hash = xxhash_rust::xxh3::xxh3_128(&contents);
    self.file_states.insert(raw_id.to_string(), FileState::Hash(hash));
    hasher.update(&[1]);
    hasher.update(&hash.to_le_bytes());
    Some(ModuleKey { key: hasher.digest128(), contents: Some(contents) })
  }

  /// The saved result of a module, newest dependency list first.
  pub async fn lookup<Fs: FileSystem + Clone + 'static>(
    &self,
    fs: &Fs,
    key1: Key,
  ) -> Option<SavedResult> {
    let store = Arc::clone(&self.cache.store);
    let lists = run_blocking(move || store.dependency_lists(key1)).await;
    for list in lists {
      let key2 = self.dependency_key(fs, &list).await;
      let store = Arc::clone(&self.cache.store);
      if let Some(bytes) = run_blocking(move || store.get(key1, key2)).await {
        if let Some(result) = SavedResult::decode(&bytes) {
          return Some(result);
        }
      }
    }
    None
  }

  /// Adds a saved result. The next `start_flush` writes it to disk.
  pub async fn save<Fs: FileSystem + Clone + 'static>(
    &self,
    fs: &Fs,
    key1: Key,
    mut dependencies: Vec<String>,
    result: &SavedResult,
  ) {
    dependencies.sort_unstable();
    dependencies.dedup();
    let dependencies: Arc<[String]> = dependencies.into();
    let key2 = self.dependency_key(fs, &dependencies).await;
    self.cache.store.put(key1, dependencies, key2, result.encode());
  }

  /// Key 2: each path of a dependency list and the state of the file at that path.
  async fn dependency_key<Fs: FileSystem + Clone + 'static>(
    &self,
    fs: &Fs,
    dependencies: &[String],
  ) -> Key {
    let mut hasher = Xxh3Default::new();
    for path in dependencies {
      hash_str(&mut hasher, path);
      if self.cache.assumed_unchanged(path) {
        hasher.update(&[2]);
      } else {
        self.file_state(fs, path).await.hash_into(&mut hasher);
      }
    }
    hasher.digest128()
  }

  async fn file_state<Fs: FileSystem + Clone + 'static>(&self, fs: &Fs, path: &str) -> FileState {
    if let Some(state) = self.file_states.get(path) {
      return *state;
    }
    let state = {
      let fs = fs.clone();
      let path = PathBuf::from(path);
      // Any read error counts as "missing", for example a directory or a file that cannot be
      // read. The state only has to change when the file changes.
      run_blocking(move || {
        fs.read(&path)
          .map_or(FileState::Missing, |bytes| FileState::Hash(xxhash_rust::xxh3::xxh3_128(&bytes)))
      })
      .await
    };
    self.file_states.insert(path.to_string(), state);
    state
  }
}

fn hash_str(hasher: &mut Xxh3Default, value: &str) {
  hasher.update(&(value.len() as u64).to_le_bytes());
  hasher.update(value.as_bytes());
}

/// Runs file system work off the async runtime, as `load_source` does.
async fn run_blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
  #[cfg(not(target_family = "wasm"))]
  {
    match tokio::runtime::Handle::try_current() {
      Ok(handle) => handle.spawn_blocking(f).await.expect("cache task should not panic"),
      Err(_) => f(),
    }
  }
  #[cfg(target_family = "wasm")]
  {
    f()
  }
}
