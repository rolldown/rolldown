//! TEMPORARY fs profiling + prefetch experiment for the apps/10000 benchmark. Not for commit.
#![allow(
  clippy::print_stderr,
  clippy::cast_possible_truncation,
  clippy::cast_precision_loss,
  clippy::missing_panics_doc
)]
use std::{
  collections::HashMap,
  hash::{BuildHasher, Hash, Hasher},
  path::{Path, PathBuf},
  sync::{
    Arc, Condvar, LazyLock, Mutex, OnceLock, RwLock,
    atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering::Relaxed},
  },
  time::Instant,
};

use oxc_resolver::{FileMetadata, FileSystemOs};

fn env_is(name: &str, val: &str) -> bool {
  std::env::var(name).is_ok_and(|v| v == val)
}

/// Experiment #3, `ROLLDOWN_FS_LIMIT=N`: at most N threads at a time inside an fs syscall.
/// The kernel benchmark peaks at 3-4 threads; rolldown calls fs from up to 25 threads.
struct FsLimit {
  max: usize,
  busy: Mutex<usize>,
  cv: Condvar,
}
static FS_LIMIT: LazyLock<Option<FsLimit>> = LazyLock::new(|| {
  let max = std::env::var("ROLLDOWN_FS_LIMIT").ok()?.parse().ok()?;
  Some(FsLimit { max, busy: Mutex::new(0), cv: Condvar::new() })
});
pub struct FsPermit(Option<&'static FsLimit>);
impl Drop for FsPermit {
  fn drop(&mut self) {
    if let Some(l) = self.0 {
      *l.busy.lock().unwrap() -= 1;
      l.cv.notify_one();
    }
  }
}
pub fn fs_permit() -> FsPermit {
  let Some(l) = FS_LIMIT.as_ref() else { return FsPermit(None) };
  let mut busy = l.busy.lock().unwrap();
  while *busy >= l.max {
    busy = l.cv.wait(busy).unwrap();
  }
  *busy += 1;
  FsPermit(Some(l))
}

/// `ROLLDOWN_FS_PROFILE=1`: per-op counters and in-flight intervals.
pub fn stats_on() -> bool {
  static ON: OnceLock<bool> = OnceLock::new();
  *ON.get_or_init(|| env_is("ROLLDOWN_FS_PROFILE", "1"))
}

pub struct OpStat {
  pub name: &'static str,
  count: AtomicU64,
  nanos: AtomicU64,
  bytes: AtomicU64,
}

impl OpStat {
  const fn new(name: &'static str) -> Self {
    Self { name, count: AtomicU64::new(0), nanos: AtomicU64::new(0), bytes: AtomicU64::new(0) }
  }
  pub fn add(&self, start: Instant, bytes: usize) {
    if !stats_on() {
      return;
    }
    self.count.fetch_add(1, Relaxed);
    self.nanos.fetch_add(start.elapsed().as_nanos() as u64, Relaxed);
    self.bytes.fetch_add(bytes as u64, Relaxed);
  }
  fn reset(&self) {
    self.count.store(0, Relaxed);
    self.nanos.store(0, Relaxed);
    self.bytes.store(0, Relaxed);
  }
}

pub static READ: OpStat = OpStat::new("read");
pub static READ_TO_STRING: OpStat = OpStat::new("read_to_string");
pub static METADATA: OpStat = OpStat::new("metadata");
pub static SYMLINK_METADATA: OpStat = OpStat::new("symlink_metadata");
pub static READ_LINK: OpStat = OpStat::new("read_link");
pub static CANONICALIZE: OpStat = OpStat::new("canonicalize");
pub static WRITE: OpStat = OpStat::new("write");
pub static OTHER: OpStat = OpStat::new("exists/mkdir/read_dir/rm");
/// Time inside the `spawn_blocking` closure of `load_source` (the read itself).
pub static LOAD_READ: OpStat = OpStat::new("load: read in closure");
/// Wall time from `spawn_blocking` call to `.await` return (read + blocking-pool queue).
pub static LOAD_AWAIT: OpStat = OpStat::new("load: spawn_blocking await");
/// One `readdir` of the `ROLLDOWN_FS_DIRCACHE` experiment.
pub static DIR_LIST: OpStat = OpStat::new("dircache: readdir");

static ALL: [&OpStat; 11] = [
  &READ,
  &READ_TO_STRING,
  &METADATA,
  &SYMLINK_METADATA,
  &READ_LINK,
  &CANONICALIZE,
  &WRITE,
  &OTHER,
  &LOAD_READ,
  &LOAD_AWAIT,
  &DIR_LIST,
];

static EPOCH: OnceLock<Instant> = OnceLock::new();
static LOAD_INTERVALS: Mutex<Vec<(u64, u64)>> = Mutex::new(Vec::new());
static ALL_INTERVALS: Mutex<Vec<(u64, u64)>> = Mutex::new(Vec::new());

fn epoch() -> Instant {
  *EPOCH.get_or_init(Instant::now)
}

fn ns(t: Instant) -> u64 {
  t.duration_since(epoch()).as_nanos() as u64
}

pub fn record_any_interval(start: Instant) {
  if !stats_on() {
    return;
  }
  let end = Instant::now();
  ALL_INTERVALS.lock().unwrap().push((ns(start), ns(end)));
}

/// Wrap the blocking read inside `load_source`.
pub fn timed_load_read<T>(
  f: impl FnOnce() -> std::io::Result<T>,
  len: impl Fn(&T) -> usize,
) -> std::io::Result<T> {
  if !stats_on() {
    return f();
  }
  let _ = epoch();
  let start = Instant::now();
  let r = f();
  let end = Instant::now();
  LOAD_READ.add(start, r.as_ref().map_or(0, &len));
  LOAD_INTERVALS.lock().unwrap().push((ns(start), ns(end)));
  r
}

// ---------------------------------------------------------------------------------------------
// Whole-process caches (earlier experiment) + trace recording.

/// `ROLLDOWN_FS_CACHE=1`: keep every `read_to_string` result in memory across builds.
pub fn cache_enabled() -> bool {
  static ON: OnceLock<bool> = OnceLock::new();
  *ON.get_or_init(|| env_is("ROLLDOWN_FS_CACHE", "1"))
}
static CACHE: OnceLock<RwLock<HashMap<PathBuf, String>>> = OnceLock::new();
pub fn cache_get(path: &Path) -> Option<String> {
  CACHE.get_or_init(Default::default).read().unwrap().get(path).cloned()
}
pub fn cache_put(path: &Path, s: &str) {
  if CACHE
    .get_or_init(Default::default)
    .write()
    .unwrap()
    .insert(path.to_path_buf(), s.to_string())
    .is_none()
  {
    TRACE.lock().unwrap().push((b'R', path.to_path_buf()));
  }
}

/// `ROLLDOWN_FS_CACHE_META=1`: keep every `symlink_metadata` result in memory across builds.
pub fn meta_cache_enabled() -> bool {
  static ON: OnceLock<bool> = OnceLock::new();
  *ON.get_or_init(|| env_is("ROLLDOWN_FS_CACHE_META", "1"))
}
type Meta = Option<FileMetadata>;
static META: OnceLock<RwLock<HashMap<PathBuf, Meta>>> = OnceLock::new();
pub fn meta_get(path: &Path) -> Option<Meta> {
  META.get_or_init(Default::default).read().unwrap().get(path).copied()
}
pub fn meta_put(path: &Path, m: Meta) {
  if META.get_or_init(Default::default).write().unwrap().insert(path.to_path_buf(), m).is_none() {
    TRACE.lock().unwrap().push((b'M', path.to_path_buf()));
  }
}

/// First-access order of every cached path. `M` = `symlink_metadata`, `R` = `read_to_string`.
static TRACE: Mutex<Vec<(u8, PathBuf)>> = Mutex::new(Vec::new());

/// `ROLLDOWN_FS_DUMP=<dir>` writes `<dir>/trace.txt` after a build (needs both caches on).
fn dump() {
  let Ok(dir) = std::env::var("ROLLDOWN_FS_DUMP") else { return };
  let trace = TRACE.lock().unwrap();
  let mut out = String::new();
  for (k, p) in trace.iter() {
    out.push(*k as char);
    out.push(' ');
    out.push_str(&p.display().to_string());
    out.push('\n');
  }
  std::fs::write(Path::new(&dir).join("trace.txt"), out).unwrap();
  eprintln!("dumped trace with {} entries", trace.len());
}

// ---------------------------------------------------------------------------------------------
// Prefetch experiment.
//
// `ROLLDOWN_FS_PREFETCH=<dir>`: `BindingBundler::new()` starts background threads that load
// `<dir>/trace.txt` and stat/read every path in it into `Store`. The resolver and the loader
// take results from `Store`; a path the build reaches first is claimed, and the prefetcher
// skips it.
// `ROLLDOWN_FS_PREFETCH_MODE=replay|meta_then_read|read_then_meta|reads|metas` (default replay)
// `ROLLDOWN_FS_PREFETCH_THREADS=N` (default 1)

enum Slot<T> {
  /// The prefetcher is reading it now.
  InFlight,
  /// The build used (or claimed) it. The prefetcher must not touch it.
  Taken,
  Ready(T),
}

struct ShardedMap<T> {
  shards: Vec<Mutex<HashMap<PathBuf, Slot<T>>>>,
  hasher: std::collections::hash_map::RandomState,
}

impl<T> ShardedMap<T> {
  fn new() -> Self {
    Self {
      shards: (0..128).map(|_| Mutex::new(HashMap::new())).collect(),
      hasher: Default::default(),
    }
  }
  fn shard(&self, p: &Path) -> &Mutex<HashMap<PathBuf, Slot<T>>> {
    let mut h = self.hasher.build_hasher();
    p.hash(&mut h);
    &self.shards[(h.finish() as usize) % self.shards.len()]
  }
}

#[derive(Default)]
struct Counters {
  text_hit: AtomicU64,
  text_miss: AtomicU64,
  text_inflight: AtomicU64,
  meta_hit: AtomicU64,
  meta_miss: AtomicU64,
  meta_inflight: AtomicU64,
  pf_done: AtomicU64,
  pf_skipped: AtomicU64,
  pf_text_n: AtomicU64,
  pf_text_nanos: AtomicU64,
}

pub struct Store {
  text: ShardedMap<String>,
  meta: ShardedMap<Meta>,
  cancelled: AtomicBool,
  t0: Instant,
  /// ns after `t0`.
  manifest_loaded_at: AtomicU64,
  finished_at: AtomicU64,
  threads_left: AtomicUsize,
  total_items: AtomicUsize,
  c: Counters,
}

static CURRENT: RwLock<Option<Arc<Store>>> = RwLock::new(None);

static ACTIVE: AtomicBool = AtomicBool::new(false);

fn current() -> Option<Arc<Store>> {
  if !ACTIVE.load(Relaxed) {
    return None;
  }
  CURRENT.read().unwrap().clone()
}

/// Called from `BindingBundler::new()`.
pub fn prefetch_start_from_env() {
  let Ok(dir) = std::env::var("ROLLDOWN_FS_PREFETCH") else { return };
  let mode = std::env::var("ROLLDOWN_FS_PREFETCH_MODE").unwrap_or_else(|_| "replay".into());
  let threads: usize =
    std::env::var("ROLLDOWN_FS_PREFETCH_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
  let store = Arc::new(Store {
    text: ShardedMap::new(),
    meta: ShardedMap::new(),
    cancelled: AtomicBool::new(false),
    t0: Instant::now(),
    manifest_loaded_at: AtomicU64::new(0),
    finished_at: AtomicU64::new(0),
    threads_left: AtomicUsize::new(threads),
    total_items: AtomicUsize::new(0),
    c: Counters::default(),
  });
  ACTIVE.store(true, Relaxed);
  if let Some(old) = CURRENT.write().unwrap().replace(Arc::clone(&store)) {
    old.cancelled.store(true, Relaxed);
  }
  std::thread::Builder::new()
    .name("rolldown-prefetch-0".into())
    .spawn(move || {
      let raw = std::fs::read_to_string(Path::new(&dir).join("trace.txt")).unwrap();
      let mut items: Vec<(u8, PathBuf)> =
        raw.lines().map(|l| (l.as_bytes()[0], PathBuf::from(&l[2..]))).collect();
      match mode.as_str() {
        "replay" => {}
        "meta_then_read" => items.sort_by_key(|(k, _)| *k != b'M'),
        "read_then_meta" => items.sort_by_key(|(k, _)| *k != b'R'),
        "reads" => items.retain(|(k, _)| *k == b'R'),
        "metas" => items.retain(|(k, _)| *k == b'M'),
        m => panic!("unknown ROLLDOWN_FS_PREFETCH_MODE {m}"),
      }
      store.total_items.store(items.len(), Relaxed);
      store.manifest_loaded_at.store(store.t0.elapsed().as_nanos() as u64, Relaxed);
      let items = Arc::new(items);
      let cursor = Arc::new(AtomicUsize::new(0));
      for i in 1..threads {
        let (store, items, cursor) = (Arc::clone(&store), Arc::clone(&items), Arc::clone(&cursor));
        std::thread::Builder::new()
          .name(format!("rolldown-prefetch-{i}"))
          .spawn(move || prefetch_worker(&store, &items, &cursor))
          .unwrap();
      }
      prefetch_worker(&store, &items, &cursor);
    })
    .unwrap();
}

fn prefetch_worker(store: &Store, items: &[(u8, PathBuf)], cursor: &AtomicUsize) {
  loop {
    if store.cancelled.load(Relaxed) {
      break;
    }
    let i = cursor.fetch_add(1, Relaxed);
    let Some((kind, path)) = items.get(i) else { break };
    let done = if *kind == b'M' {
      prefetch_one(&store.meta, path, || FileSystemOs::symlink_metadata(path).ok())
    } else {
      prefetch_one(&store.text, path, || {
        let t = Instant::now();
        let r = FileSystemOs::read_to_string(path).ok().unwrap_or_default();
        store.c.pf_text_n.fetch_add(1, Relaxed);
        store.c.pf_text_nanos.fetch_add(t.elapsed().as_nanos() as u64, Relaxed);
        r
      })
    };
    if done { &store.c.pf_done } else { &store.c.pf_skipped }.fetch_add(1, Relaxed);
  }
  if store.threads_left.fetch_sub(1, Relaxed) == 1 {
    store.finished_at.store(store.t0.elapsed().as_nanos() as u64, Relaxed);
  }
}

fn prefetch_one<T>(map: &ShardedMap<T>, path: &Path, f: impl FnOnce() -> T) -> bool {
  {
    let mut shard = map.shard(path).lock().unwrap();
    if shard.contains_key(path) {
      return false;
    }
    shard.insert(path.to_path_buf(), Slot::InFlight);
  }
  let v = f();
  let mut shard = map.shard(path).lock().unwrap();
  if let Some(slot @ Slot::InFlight) = shard.get_mut(path) {
    *slot = Slot::Ready(v);
  }
  true
}

/// Loader side. `Some` = prefetched content (moved out). `None` = read it yourself.
pub fn prefetch_take_text(path: &Path) -> Option<String> {
  let store = current()?;
  let mut shard = store.text.shard(path).lock().unwrap();
  match shard.insert(path.to_path_buf(), Slot::Taken) {
    Some(Slot::Ready(s)) => {
      store.c.text_hit.fetch_add(1, Relaxed);
      Some(s)
    }
    Some(Slot::InFlight) => {
      // Now `Taken`, so the prefetcher drops its result when the read completes.
      store.c.text_inflight.fetch_add(1, Relaxed);
      None
    }
    _ => {
      store.c.text_miss.fetch_add(1, Relaxed);
      None
    }
  }
}

/// Resolver side. `Some(m)` = prefetched result. `None` = do the syscall yourself.
pub fn prefetch_get_meta(path: &Path) -> Option<Meta> {
  let store = current()?;
  let mut shard = store.meta.shard(path).lock().unwrap();
  match shard.get(path) {
    Some(Slot::Ready(m)) => {
      store.c.meta_hit.fetch_add(1, Relaxed);
      Some(*m)
    }
    Some(Slot::InFlight) => {
      store.c.meta_inflight.fetch_add(1, Relaxed);
      None
    }
    Some(Slot::Taken) => None,
    None => {
      shard.insert(path.to_path_buf(), Slot::Taken);
      store.c.meta_miss.fetch_add(1, Relaxed);
      None
    }
  }
}

fn prefetch_report(build_start: Instant) {
  let Some(store) = current() else { return };
  store.cancelled.store(true, Relaxed);
  let ms = |n: u64| n as f64 / 1e6;
  let start_off = build_start.duration_since(store.t0).as_nanos() as u64;
  let c = &store.c;
  let fin = store.finished_at.load(Relaxed);
  eprintln!(
    "prefetch: started {:.1} ms before build; manifest at +{:.1} ms; finished {}; items {} done {} skipped {}",
    ms(start_off),
    ms(store.manifest_loaded_at.load(Relaxed)),
    if fin == 0 {
      "NO (cancelled at build end)".to_string()
    } else {
      format!("at +{:.1} ms", ms(fin))
    },
    store.total_items.load(Relaxed),
    c.pf_done.load(Relaxed),
    c.pf_skipped.load(Relaxed),
  );
  eprintln!(
    "prefetch: text hit {} miss {} inflight {}; meta hit {} miss {} inflight {}",
    c.text_hit.load(Relaxed),
    c.text_miss.load(Relaxed),
    c.text_inflight.load(Relaxed),
    c.meta_hit.load(Relaxed),
    c.meta_miss.load(Relaxed),
    c.meta_inflight.load(Relaxed),
  );
  let n = c.pf_text_n.load(Relaxed);
  eprintln!(
    "prefetch: warmup reads {} avg {:.1} us",
    n,
    c.pf_text_nanos.load(Relaxed) as f64 / n.max(1) as f64 / 1e3
  );
}

// ---------------------------------------------------------------------------------------------
// Warm fs caches. One `Warm` value per build holds both caches below.
//
// Experiment #1, `ROLLDOWN_FS_DIRCACHE=1|2`: answer `symlink_metadata` from one listing per
// directory (file type from `d_type`), so the 52k probes for missing paths cost no syscall.
// `1` = wait for a listing that another thread is reading now; `2` = do an `lstat` instead.
// `ROLLDOWN_FS_DIRCACHE_PERSIST=<file>` keeps the listings between builds. A stored listing is
// used when one `stat` of the directory gives the same stamp: adding, removing or renaming an
// entry changes the directory's mtime and ctime.
// Caveat: names match by exact case, but APFS and NTFS `lstat` are case-insensitive by default.
//
// Experiment #4, `ROLLDOWN_FS_CONTENT_CACHE=<file>`: keep the content of every file that the
// loader and the resolver (`package.json`) read, with a stamp per file, like git's index. A read
// is a `stat` + stamp compare. A miss opens the file and takes its stamp with `fstat` on the same
// handle, before the read, so a change during the read shows up next build.
// `ROLLDOWN_FS_CONTENT_CACHE_POOL=1` checks in the blocking pool, not on the tokio worker.
//
// `ROLLDOWN_FS_CONTENT_CACHE_COLD=1` ignores both stored files (measures a cold build + save).
// `ROLLDOWN_FS_WARM_EARLY=1`: `BindingBundler::new()` (the `rolldown()` call) loads both files and
// checks every stored stamp on `ROLLDOWN_FS_WARM_THREADS` (default 4) threads.
// A file or directory changed less than 2 s ago is not stored (git's "racily clean" problem).
// Both files are written on a background thread at build end; the next load waits for it.

/// dev, ino, size, mtime s, mtime ns, ctime s, ctime ns.
/// On Windows: 0, 0, size, last write s, ns, creation s, ns (the fields git for Windows uses).
pub type Stamp = [i64; 7];
const CC_MAGIC: &[u8; 8] = b"RDCC0002";
const DC_MAGIC: &[u8; 8] = b"RDDC0001";
/// Mode 3 file: per directory, only the names that were probed (kind byte `MISSING` = absent).
const DP_MAGIC: &[u8; 8] = b"RDDP0001";
/// Kind bits: 1 file, 2 dir, 4 symlink, 8 missing (mode 3 only).
const MISSING: u8 = 8;
const UNKNOWN: u8 = 0;
const VALID: u8 = 1;
const STALE: u8 = 2;

/// `1` = a thread that needs a directory another thread is listing waits for that listing.
/// `2` = it does its own `lstat` instead (no stall behind a big `readdir`).
/// `3` = no `readdir`: record each probe result per directory, checked by one `stat` of the directory.
fn dircache_mode() -> u8 {
  static MODE: OnceLock<u8> = OnceLock::new();
  *MODE.get_or_init(|| {
    if !STAMPS_SUPPORTED {
      return 0;
    }
    match std::env::var("ROLLDOWN_FS_DIRCACHE").as_deref() {
      Ok("1") => 1,
      Ok("2") => 2,
      Ok("3") => 3,
      _ => 0,
    }
  })
}

pub fn dircache_enabled() -> bool {
  dircache_mode() != 0
}

fn dir_persist_path() -> Option<&'static PathBuf> {
  static P: OnceLock<Option<PathBuf>> = OnceLock::new();
  P.get_or_init(|| {
    if !dircache_enabled() {
      return None;
    }
    std::env::var_os("ROLLDOWN_FS_DIRCACHE_PERSIST").map(PathBuf::from)
  })
  .as_ref()
}

fn cc_path() -> Option<&'static PathBuf> {
  static P: OnceLock<Option<PathBuf>> = OnceLock::new();
  P.get_or_init(|| {
    if !STAMPS_SUPPORTED {
      return None;
    }
    std::env::var_os("ROLLDOWN_FS_CONTENT_CACHE").map(PathBuf::from)
  })
  .as_ref()
}

pub fn content_cache_enabled() -> bool {
  cc_path().is_some()
}

pub fn content_cache_in_pool() -> bool {
  static ON: OnceLock<bool> = OnceLock::new();
  *ON.get_or_init(|| env_is("ROLLDOWN_FS_CONTENT_CACHE_POOL", "1"))
}

fn cold() -> bool {
  env_is("ROLLDOWN_FS_CONTENT_CACHE_COLD", "1")
}

#[cfg(unix)]
fn stamp_from(m: &std::fs::Metadata) -> Stamp {
  use std::os::unix::fs::MetadataExt;
  [
    m.dev() as i64,
    m.ino() as i64,
    m.size() as i64,
    m.mtime(),
    m.mtime_nsec(),
    m.ctime(),
    m.ctime_nsec(),
  ]
}
/// Stable std has no inode or change time on Windows, so the stamp has size, write and creation time.
#[cfg(windows)]
fn stamp_from(m: &std::fs::Metadata) -> Stamp {
  use std::os::windows::fs::MetadataExt;
  /// `FILETIME` counts 100 ns ticks from 1601-01-01; the stamp counts from 1970-01-01.
  fn unix_time(ticks: u64) -> (i64, i64) {
    let t = ticks.cast_signed() - 116_444_736_000_000_000;
    (t.div_euclid(10_000_000), t.rem_euclid(10_000_000) * 100)
  }
  let (ms, mns) = unix_time(m.last_write_time());
  let (cs, cns) = unix_time(m.creation_time());
  [0, 0, m.file_size().cast_signed(), ms, mns, cs, cns]
}
#[cfg(not(any(unix, windows)))]
fn stamp_from(_m: &std::fs::Metadata) -> Stamp {
  [0; 7]
}

/// Other targets (wasi) have no stamp, so they ignore the cache env vars.
const STAMPS_SUPPORTED: bool = cfg!(any(unix, windows));

#[cfg(unix)]
fn os_str_bytes(s: &std::ffi::OsStr) -> std::borrow::Cow<'_, [u8]> {
  use std::os::unix::ffi::OsStrExt;
  std::borrow::Cow::Borrowed(s.as_bytes())
}
/// UTF-16 code units, so a name that is not valid Unicode keeps its own key.
#[cfg(windows)]
fn os_str_bytes(s: &std::ffi::OsStr) -> std::borrow::Cow<'_, [u8]> {
  use std::os::windows::ffi::OsStrExt;
  std::borrow::Cow::Owned(s.encode_wide().flat_map(u16::to_le_bytes).collect())
}
#[cfg(not(any(unix, windows)))]
fn os_str_bytes(s: &std::ffi::OsStr) -> std::borrow::Cow<'_, [u8]> {
  std::borrow::Cow::Borrowed(s.to_str().unwrap_or_default().as_bytes())
}

#[cfg(unix)]
fn os_string_from_bytes(b: &[u8]) -> std::ffi::OsString {
  use std::os::unix::ffi::OsStrExt;
  std::ffi::OsStr::from_bytes(b).to_os_string()
}
#[cfg(windows)]
fn os_string_from_bytes(b: &[u8]) -> std::ffi::OsString {
  use std::os::windows::ffi::OsStringExt;
  let units: Vec<u16> = b.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
  std::ffi::OsString::from_wide(&units)
}
#[cfg(not(any(unix, windows)))]
fn os_string_from_bytes(b: &[u8]) -> std::ffi::OsString {
  String::from_utf8_lossy(b).into_owned().into()
}

fn stamp_of(path: &Path) -> Option<Stamp> {
  std::fs::metadata(path).ok().map(|m| stamp_from(&m))
}

/// FAT stores mtime in 2 s steps, so a shorter window can store a file that changes again with the
/// same stamp.
const RACY_WINDOW_NS: i128 = 2_000_000_000;

/// Some file systems (FUSE) report mtime 0, so the stamp cannot show an edit that keeps the size.
fn is_hot(s: &Stamp) -> bool {
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map_or(0, |d| d.as_nanos() as i128);
  let mtime = i128::from(s[3]) * 1_000_000_000 + i128::from(s[4]);
  mtime <= 0 || now - mtime < RACY_WINDOW_NS
}

/// The early check result if there is one, else one `stat` now.
fn still_valid(path: &Path, stamp: &Stamp, state: &AtomicU8) -> bool {
  match state.load(Relaxed) {
    VALID => true,
    STALE => false,
    _ => stamp_of(path).as_ref() == Some(stamp),
  }
}

struct Reader<'a> {
  buf: &'a [u8],
  p: usize,
}
impl<'a> Reader<'a> {
  fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
    let s = self.buf.get(self.p..self.p + n)?;
    self.p += n;
    Some(s)
  }
  fn u8(&mut self) -> Option<u8> {
    Some(self.bytes(1)?[0])
  }
  fn u16(&mut self) -> Option<u16> {
    Some(u16::from_le_bytes(self.bytes(2)?.try_into().ok()?))
  }
  fn u32(&mut self) -> Option<u32> {
    Some(u32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
  }
  fn u64(&mut self) -> Option<u64> {
    Some(u64::from_le_bytes(self.bytes(8)?.try_into().ok()?))
  }
  fn stamp(&mut self) -> Option<Stamp> {
    let mut s = [0i64; 7];
    for v in &mut s {
      *v = i64::from_le_bytes(self.bytes(8)?.try_into().ok()?);
    }
    Some(s)
  }
  fn path(&mut self) -> Option<PathBuf> {
    let n = self.u32()? as usize;
    Some(PathBuf::from(os_string_from_bytes(self.bytes(n)?)))
  }
}

#[derive(Default)]
struct Writer(Vec<u8>);
impl Writer {
  fn bytes(&mut self, b: &[u8]) {
    self.0.extend_from_slice(b);
  }
  fn stamp(&mut self, s: &Stamp) {
    for v in s {
      self.bytes(&v.to_le_bytes());
    }
  }
  fn path(&mut self, p: &Path) {
    let b = os_str_bytes(p.as_os_str());
    self.bytes(&(b.len() as u32).to_le_bytes());
    self.bytes(&b);
  }
}

/// A failed save only costs the next build its cache. On Windows, a scanner can hold the file open.
fn write_atomic(dst: &Path, data: &[u8]) {
  let tmp = dst.with_extension("tmp");
  if std::fs::write(&tmp, data).and_then(|()| std::fs::rename(&tmp, dst)).is_err() {
    let _ = std::fs::remove_file(&tmp);
  }
}

// ---- stored content

struct CcEntry {
  stamp: Stamp,
  off: usize,
  len: usize,
  used: AtomicBool,
  state: AtomicU8,
}

struct CcState {
  buf: Vec<u8>,
  map: HashMap<PathBuf, CcEntry>,
  load_ns: u64,
}

fn cc_load() -> CcState {
  let t = Instant::now();
  let buf = if cold() { Vec::new() } else { std::fs::read(cc_path().unwrap()).unwrap_or_default() };
  let map = cc_parse(&buf).unwrap_or_default();
  CcState { buf, map, load_ns: t.elapsed().as_nanos() as u64 }
}

fn cc_parse(buf: &[u8]) -> Option<HashMap<PathBuf, CcEntry>> {
  let mut r = Reader { buf, p: 0 };
  if r.bytes(8)? != CC_MAGIC {
    return None;
  }
  let n = r.u64()? as usize;
  let mut map = HashMap::with_capacity(n);
  for _ in 0..n {
    let path = r.path()?;
    let stamp = r.stamp()?;
    let len = r.u32()? as usize;
    let off = r.p;
    r.bytes(len)?;
    map.insert(
      path,
      CcEntry { stamp, off, len, used: AtomicBool::new(false), state: AtomicU8::new(UNKNOWN) },
    );
  }
  Some(map)
}

// ---- stored directory listings

type Listing = HashMap<std::ffi::OsString, u8>;

fn kind_of(m: &FileMetadata) -> u8 {
  u8::from(m.is_file()) | u8::from(m.is_dir()) << 1 | u8::from(m.is_symlink()) << 2
}

fn meta_of(k: u8) -> std::io::Result<FileMetadata> {
  if k & MISSING != 0 {
    return Err(std::io::Error::from(std::io::ErrorKind::NotFound));
  }
  Ok(FileMetadata::new(k & 1 != 0, k & 2 != 0, k & 4 != 0))
}

struct StoredDir {
  stamp: Stamp,
  listing: Arc<Listing>,
  state: AtomicU8,
}

struct DirState {
  map: HashMap<PathBuf, StoredDir>,
  bytes: usize,
  load_ns: u64,
}

fn dirs_load() -> DirState {
  let t = Instant::now();
  let buf = if cold() {
    Vec::new()
  } else {
    std::fs::read(dir_persist_path().unwrap()).unwrap_or_default()
  };
  let map = dirs_parse(&buf).unwrap_or_default();
  DirState { map, bytes: buf.len(), load_ns: t.elapsed().as_nanos() as u64 }
}

fn dirs_parse(buf: &[u8]) -> Option<HashMap<PathBuf, StoredDir>> {
  let mut r = Reader { buf, p: 0 };
  if r.bytes(8)? != if dircache_mode() == 3 { DP_MAGIC } else { DC_MAGIC } {
    return None;
  }
  let n = r.u64()? as usize;
  let mut map = HashMap::with_capacity(n);
  for _ in 0..n {
    let path = r.path()?;
    let stamp = r.stamp()?;
    let count = r.u32()? as usize;
    let mut listing = Listing::with_capacity(count);
    for _ in 0..count {
      let kind = r.u8()?;
      let len = r.u16()? as usize;
      let name = os_string_from_bytes(r.bytes(len)?);
      listing.insert(name, kind);
    }
    map
      .insert(path, StoredDir { stamp, listing: Arc::new(listing), state: AtomicU8::new(UNKNOWN) });
  }
  Some(map)
}

// ---- per-build state

#[derive(Default)]
struct DirSlot {
  started: AtomicBool,
  /// `None` = the directory does not exist (or cannot be listed).
  done: OnceLock<Option<Arc<Listing>>>,
  /// Set when the listing may be stored for the next build.
  stamp: OnceLock<Stamp>,
  /// Mode 3.
  rec: OnceLock<Rec>,
}

/// Mode 3: probe results for one directory in this build.
#[derive(Default)]
struct Rec {
  /// The directory stamp, taken before the first probe. `None` = do not store (hot or missing).
  stamp: Option<Stamp>,
  /// The stored record, when its stamp still matches.
  stored: Option<Arc<Listing>>,
  /// `stat` of the directory failed, so no child exists.
  missing: bool,
  /// Results found by syscalls in this build.
  new: Mutex<Listing>,
}

#[derive(Default)]
struct WarmCounters {
  hit: AtomicU64,
  changed: AtomicU64,
  absent: AtomicU64,
  hot: AtomicU64,
  dir_valid: AtomicU64,
  dir_read: AtomicU64,
  dir_hot: AtomicU64,
  busy: AtomicU64,
  dir_missing: AtomicU64,
  probe_hit: AtomicU64,
  probe_sys: AtomicU64,
  recorded: AtomicU64,
  early_checked: AtomicU64,
  early_stale: AtomicU64,
  early_ns: AtomicU64,
}

struct Warm {
  t0: Instant,
  content: OnceLock<CcState>,
  dirs: OnceLock<DirState>,
  fresh: Mutex<Vec<(PathBuf, Stamp, Vec<u8>)>>,
  slots: Vec<Mutex<HashMap<PathBuf, Arc<DirSlot>>>>,
  hasher: std::collections::hash_map::RandomState,
  c: WarmCounters,
}

impl Warm {
  fn new() -> Self {
    Self {
      t0: Instant::now(),
      content: OnceLock::new(),
      dirs: OnceLock::new(),
      fresh: Mutex::default(),
      slots: (0..128).map(|_| Mutex::new(HashMap::new())).collect(),
      hasher: Default::default(),
      c: WarmCounters::default(),
    }
  }
  fn content(&self) -> &CcState {
    self.content.get_or_init(cc_load)
  }
  fn dirs(&self) -> Option<&DirState> {
    dir_persist_path()?;
    Some(self.dirs.get_or_init(dirs_load))
  }
  fn slot_if_exists(&self, dir: &Path) -> Option<Arc<DirSlot>> {
    let mut h = self.hasher.build_hasher();
    dir.hash(&mut h);
    self.slots[(h.finish() as usize) % self.slots.len()].lock().unwrap().get(dir).cloned()
  }
  fn slot(&self, dir: &Path) -> Arc<DirSlot> {
    let mut h = self.hasher.build_hasher();
    dir.hash(&mut h);
    let mut shard = self.slots[(h.finish() as usize) % self.slots.len()].lock().unwrap();
    match shard.get(dir) {
      Some(slot) => Arc::clone(slot),
      None => Arc::clone(shard.entry(dir.to_path_buf()).or_default()),
    }
  }
}

static WARM: RwLock<Option<Arc<Warm>>> = RwLock::new(None);
/// Started by `warm_start_early()`, taken by the next `reset()`.
static NEXT: Mutex<Option<Arc<Warm>>> = Mutex::new(None);
static SAVING: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

fn warm() -> Arc<Warm> {
  if let Some(w) = WARM.read().unwrap().as_ref() {
    return Arc::clone(w);
  }
  Arc::clone(WARM.write().unwrap().get_or_insert_with(|| Arc::new(Warm::new())))
}

fn wait_for_save() {
  let h = SAVING.lock().unwrap().take();
  if let Some(h) = h {
    let _ = h.join();
  }
}

/// Load the stored files on two threads, so the directory listings do not wait for the content.
fn spawn_loads(w: &Arc<Warm>) {
  if content_cache_enabled() {
    let w = Arc::clone(w);
    std::thread::spawn(move || {
      w.content();
    });
  }
  if dir_persist_path().is_some() {
    let w = Arc::clone(w);
    std::thread::spawn(move || {
      w.dirs();
    });
  }
}

/// Called from `BindingBundler::new()`.
pub fn warm_start_early() {
  if !env_is("ROLLDOWN_FS_WARM_EARLY", "1")
    || !(content_cache_enabled() || dir_persist_path().is_some())
  {
    return;
  }
  let threads: usize =
    std::env::var("ROLLDOWN_FS_WARM_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
  let w = Arc::new(Warm::new());
  *NEXT.lock().unwrap() = Some(Arc::clone(&w));
  std::thread::Builder::new()
    .name("rolldown-warm".into())
    .spawn(move || {
      wait_for_save();
      spawn_loads(&w);
      // Directories first: the resolver needs them before the loader needs content.
      let mut jobs: Vec<(&Path, &Stamp, &AtomicU8)> = Vec::new();
      if let Some(d) = w.dirs() {
        jobs.extend(d.map.iter().map(|(p, e)| (p.as_path(), &e.stamp, &e.state)));
      }
      if content_cache_enabled() {
        jobs.extend(w.content().map.iter().map(|(p, e)| (p.as_path(), &e.stamp, &e.state)));
      }
      let next = AtomicUsize::new(0);
      std::thread::scope(|s| {
        for _ in 0..threads {
          s.spawn(|| {
            while let Some((p, stamp, state)) = jobs.get(next.fetch_add(1, Relaxed)) {
              if state.load(Relaxed) != UNKNOWN {
                continue;
              }
              let ok = stamp_of(p).as_ref() == Some(*stamp);
              state.store(if ok { VALID } else { STALE }, Relaxed);
              w.c.early_checked.fetch_add(1, Relaxed);
              if !ok {
                w.c.early_stale.fetch_add(1, Relaxed);
              }
            }
          });
        }
      });
      w.c.early_ns.store(w.t0.elapsed().as_nanos() as u64, Relaxed);
    })
    .unwrap();
}

/// Called from `reset()` at build start.
fn warm_start_build() {
  if let Some(w) = NEXT.lock().unwrap().take() {
    *WARM.write().unwrap() = Some(w);
    return;
  }
  *WARM.write().unwrap() = None;
  if content_cache_enabled() || dir_persist_path().is_some() {
    wait_for_save();
    spawn_loads(&warm());
  }
}

// ---- directory listings

enum Lookup {
  Ready(Option<Arc<Listing>>),
  /// Another thread is listing it now (mode 2 only).
  Busy,
}

fn listing(w: &Warm, dir: &Path) -> Lookup {
  let slot = w.slot(dir);
  if let Some(v) = slot.done.get() {
    return Lookup::Ready(v.clone());
  }
  if slot.started.swap(true, std::sync::atomic::Ordering::AcqRel) {
    if dircache_mode() == 1 {
      return Lookup::Ready(slot.done.wait().clone());
    }
    return Lookup::Busy;
  }
  let v = list_dir(w, dir, &slot);
  let _ = slot.done.set(v.clone());
  Lookup::Ready(v)
}

fn list_dir(w: &Warm, dir: &Path, slot: &DirSlot) -> Option<Arc<Listing>> {
  // The parent listing can prove that `dir` is missing without a syscall.
  if let (Some(parent), Some(name)) = (dir.parent(), dir.file_name())
    && let Lookup::Ready(p) = listing(w, parent)
  {
    match p?.get(name) {
      None => return None,
      Some(k) if k & 1 != 0 => return None,
      _ => {}
    }
  }
  if let Some(d) = w.dirs().and_then(|d| d.map.get(dir))
    && still_valid(dir, &d.stamp, &d.state)
  {
    let _ = slot.stamp.set(d.stamp);
    w.c.dir_valid.fetch_add(1, Relaxed);
    return Some(Arc::clone(&d.listing));
  }
  // The stamp before the listing, so a change during `readdir` shows up next build.
  let stamp = if dir_persist_path().is_some() { stamp_of(dir) } else { None };
  let t = Instant::now();
  let rd = std::fs::read_dir(dir).ok()?;
  let mut map = Listing::new();
  for e in rd.flatten() {
    if let Ok(ft) = e.file_type() {
      map.insert(
        e.file_name(),
        u8::from(ft.is_file()) | u8::from(ft.is_dir()) << 1 | u8::from(ft.is_symlink()) << 2,
      );
    }
  }
  DIR_LIST.add(t, map.len());
  record_any_interval(t);
  w.c.dir_read.fetch_add(1, Relaxed);
  if map.len() > 5000 && times_on() {
    eprintln!(
      "dircache: readdir {} entries in {:.1} ms: {}",
      map.len(),
      t.elapsed().as_secs_f64() * 1e3,
      dir.display()
    );
  }
  match stamp {
    Some(s) if !is_hot(&s) => {
      let _ = slot.stamp.set(s);
    }
    Some(_) => {
      w.c.dir_hot.fetch_add(1, Relaxed);
    }
    None => {}
  }
  Some(Arc::new(map))
}

/// Mode 3: what this build already knows about `path`, with no syscall and no new record.
fn known_kind(w: &Warm, path: &Path) -> Option<u8> {
  let (parent, name) = (path.parent()?, path.file_name()?);
  let r = w.slot_if_exists(parent)?;
  let r = r.rec.get()?;
  rec_get(r, name).or_else(|| r.new.lock().unwrap().get(name).copied())
}

fn rec_init(w: &Warm, dir: &Path) -> Rec {
  // The parent record can prove that `dir` is not a directory (missing, or a file).
  if let Some(k) = known_kind(w, dir)
    && (k & MISSING != 0 || k & 6 == 0)
  {
    w.c.dir_missing.fetch_add(1, Relaxed);
    return Rec { missing: true, ..Rec::default() };
  }
  if let Some(d) = w.dirs().and_then(|d| d.map.get(dir))
    && still_valid(dir, &d.stamp, &d.state)
  {
    w.c.dir_valid.fetch_add(1, Relaxed);
    return Rec { stamp: Some(d.stamp), stored: Some(Arc::clone(&d.listing)), ..Rec::default() };
  }
  match std::fs::metadata(dir).ok().filter(std::fs::Metadata::is_dir).map(|m| stamp_from(&m)) {
    None => {
      w.c.dir_missing.fetch_add(1, Relaxed);
      Rec { missing: true, ..Rec::default() }
    }
    Some(s) if is_hot(&s) => {
      w.c.dir_hot.fetch_add(1, Relaxed);
      Rec::default()
    }
    Some(s) => {
      w.c.dir_read.fetch_add(1, Relaxed);
      Rec { stamp: Some(s), ..Rec::default() }
    }
  }
}

fn rec<'a>(w: &Warm, slot: &'a DirSlot, dir: &Path) -> &'a Rec {
  slot.rec.get_or_init(|| rec_init(w, dir))
}

/// Mode 3: the stored answer for `name`, if there is one.
fn rec_get(r: &Rec, name: &std::ffi::OsStr) -> Option<u8> {
  if r.missing {
    return Some(MISSING);
  }
  r.stored.as_ref()?.get(name).copied()
}

fn rec_put(w: &Warm, r: &Rec, name: &std::ffi::OsStr, kind: u8) {
  if r.stamp.is_some() {
    r.new.lock().unwrap().insert(name.to_os_string(), kind);
    w.c.recorded.fetch_add(1, Relaxed);
  }
}

fn probe_symlink_metadata(
  path: &Path,
  parent: &Path,
  name: &std::ffi::OsStr,
) -> std::io::Result<FileMetadata> {
  let w = warm();
  let slot = w.slot(parent);
  let r = rec(&w, &slot, parent);
  if let Some(k) = rec_get(r, name) {
    w.c.probe_hit.fetch_add(1, Relaxed);
    return meta_of(k);
  }
  w.c.probe_sys.fetch_add(1, Relaxed);
  let res = FileSystemOs::symlink_metadata(path);
  match &res {
    Ok(m) => rec_put(&w, r, name, kind_of(m)),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => rec_put(&w, r, name, MISSING),
    Err(_) => {}
  }
  res
}

pub fn dircache_symlink_metadata(path: &Path) -> std::io::Result<FileMetadata> {
  let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
    return FileSystemOs::symlink_metadata(path);
  };
  if dircache_mode() == 3 {
    return probe_symlink_metadata(path, parent, name);
  }
  let w = warm();
  match listing(&w, parent) {
    Lookup::Ready(l) => l
      .and_then(|l| l.get(name).copied())
      .map_or_else(|| Err(std::io::Error::from(std::io::ErrorKind::NotFound)), meta_of),
    Lookup::Busy => {
      w.c.busy.fetch_add(1, Relaxed);
      FileSystemOs::symlink_metadata(path)
    }
  }
}

/// True when the directory cache proves that `path` does not exist (no syscall).
pub fn dircache_says_missing(path: &Path) -> bool {
  if !dircache_enabled() {
    return false;
  }
  let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else { return false };
  let w = warm();
  if dircache_mode() == 3 {
    let slot = w.slot(parent);
    return rec_get(rec(&w, &slot, parent), name) == Some(MISSING);
  }
  match listing(&w, parent) {
    Lookup::Ready(Some(l)) => !l.contains_key(name),
    Lookup::Ready(None) => true,
    Lookup::Busy => false,
  }
}

/// Mode 3: a read found no file, so record `path` as missing.
pub fn dircache_note_missing(path: &Path) {
  if dircache_mode() != 3 {
    return;
  }
  let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else { return };
  let w = warm();
  let slot = w.slot(parent);
  // The stamp was taken in `dircache_says_missing`, before the read.
  if let Some(r) = slot.rec.get() {
    rec_put(&w, r, name, MISSING);
  }
}

// ---- file content

fn cc_get<T>(path: &Path, conv: impl FnOnce(&[u8]) -> Option<T>) -> Option<T> {
  let w = warm();
  let st = w.content();
  let Some(e) = st.map.get(path) else {
    w.c.absent.fetch_add(1, Relaxed);
    return None;
  };
  if still_valid(path, &e.stamp, &e.state)
    && let Some(v) = conv(&st.buf[e.off..e.off + e.len])
  {
    e.used.store(true, Relaxed);
    w.c.hit.fetch_add(1, Relaxed);
    return Some(v);
  }
  w.c.changed.fetch_add(1, Relaxed);
  None
}

pub fn content_cache_get_text(path: &Path) -> Option<String> {
  cc_get(path, |b| std::str::from_utf8(b).ok().map(str::to_owned))
}

pub fn content_cache_get_bytes(path: &Path) -> Option<Vec<u8>> {
  cc_get(path, |b| Some(b.to_vec()))
}

/// A miss: read the file, take its stamp with `fstat` on the same handle, and store both.
pub fn content_cache_read_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
  use std::io::Read;
  let mut f = std::fs::File::open(path)?;
  let m = f.metadata()?;
  let mut buf = Vec::with_capacity(m.len() as usize);
  f.read_to_end(&mut buf)?;
  let stamp = stamp_from(&m);
  let w = warm();
  if is_hot(&stamp) {
    w.c.hot.fetch_add(1, Relaxed);
  } else {
    w.fresh.lock().unwrap().push((path.to_path_buf(), stamp, buf.clone()));
  }
  Ok(buf)
}

pub fn content_cache_read_text(path: &Path) -> std::io::Result<String> {
  String::from_utf8(content_cache_read_bytes(path)?).map_err(|_| {
    std::io::Error::new(std::io::ErrorKind::InvalidData, "stream did not contain valid UTF-8")
  })
}

// ---- save at build end

/// Start writing the files this build changed on a background thread. Returns a report line.
fn warm_save() -> Option<String> {
  let w = WARM.read().unwrap().clone()?;
  let ms = |n: u64| n as f64 / 1e6;
  let c = &w.c;
  let mut line = String::from("warm:");
  let mut save_content = false;
  let mut dirs_out: Vec<(PathBuf, Stamp, Arc<Listing>)> = Vec::new();
  let mut save_dirs = false;
  if let Some(st) = w.content.get() {
    let used = st.map.values().filter(|e| e.used.load(Relaxed)).count();
    save_content = !w.fresh.lock().unwrap().is_empty() || used != st.map.len();
    line.push_str(&format!(
      " content load {:.1} ms ({:.1} MB, {} files) hit {} changed {} absent {} hot {};",
      ms(st.load_ns),
      st.buf.len() as f64 / 1e6,
      st.map.len(),
      c.hit.load(Relaxed),
      c.changed.load(Relaxed),
      c.absent.load(Relaxed),
      c.hot.load(Relaxed),
    ));
  }
  if dircache_enabled() {
    for shard in &w.slots {
      for (p, slot) in shard.lock().unwrap().iter() {
        if dircache_mode() == 3 {
          if let Some(r) = slot.rec.get()
            && let Some(s) = r.stamp
          {
            let mut l = r.stored.as_deref().cloned().unwrap_or_default();
            l.extend(r.new.lock().unwrap().drain());
            dirs_out.push((p.clone(), s, Arc::new(l)));
          }
        } else if let (Some(Some(l)), Some(s)) = (slot.done.get(), slot.stamp.get()) {
          dirs_out.push((p.clone(), *s, Arc::clone(l)));
        }
      }
    }
    let stored = w.dirs.get().map_or(0, |d| d.map.len());
    let changed =
      if dircache_mode() == 3 { c.recorded.load(Relaxed) } else { c.dir_read.load(Relaxed) };
    save_dirs = dir_persist_path().is_some() && (changed > 0 || dirs_out.len() != stored);
    line.push_str(&format!(
      " dirs stored {} valid {} read {} hot {} busy {} missing {}",
      stored,
      c.dir_valid.load(Relaxed),
      c.dir_read.load(Relaxed),
      c.dir_hot.load(Relaxed),
      c.busy.load(Relaxed),
      c.dir_missing.load(Relaxed),
    ));
    if dircache_mode() == 3 {
      line.push_str(&format!(
        " probes hit {} syscall {} recorded {}",
        c.probe_hit.load(Relaxed),
        c.probe_sys.load(Relaxed),
        c.recorded.load(Relaxed),
      ));
    }
    if let Some(d) = w.dirs.get() {
      line.push_str(&format!(" (load {:.1} ms, {:.2} MB)", ms(d.load_ns), d.bytes as f64 / 1e6));
    }
    line.push(';');
  }
  if c.early_checked.load(Relaxed) > 0 || c.early_ns.load(Relaxed) > 0 {
    let done = c.early_ns.load(Relaxed);
    line.push_str(&format!(
      " early checked {} stale {} done {};",
      c.early_checked.load(Relaxed),
      c.early_stale.load(Relaxed),
      if done == 0 { "NO".to_string() } else { format!("at +{:.1} ms", ms(done)) },
    ));
  }
  line.push_str(&format!(" save content {save_content} dirs {save_dirs}"));
  if env_is("ROLLDOWN_FS_NO_SAVE", "1") {
    line.push_str(" (skipped)");
    return Some(line);
  }
  if save_content || save_dirs {
    let w = Arc::clone(&w);
    let h = std::thread::spawn(move || {
      if save_content {
        let st = w.content();
        let fresh = std::mem::take(&mut *w.fresh.lock().unwrap());
        let used: Vec<_> = st.map.iter().filter(|(_, e)| e.used.load(Relaxed)).collect();
        let mut out = Writer(Vec::with_capacity(
          st.buf.len() + fresh.iter().map(|f| f.2.len() + 100).sum::<usize>(),
        ));
        out.bytes(CC_MAGIC);
        out.bytes(&((used.len() + fresh.len()) as u64).to_le_bytes());
        let mut put = |p: &Path, s: &Stamp, content: &[u8]| {
          out.path(p);
          out.stamp(s);
          out.bytes(&(content.len() as u32).to_le_bytes());
          out.bytes(content);
        };
        for (p, e) in &used {
          put(p, &e.stamp, &st.buf[e.off..e.off + e.len]);
        }
        for (p, s, content) in &fresh {
          put(p, s, content);
        }
        write_atomic(cc_path().unwrap(), &out.0);
      }
      if save_dirs {
        let mut out = Writer::default();
        out.bytes(if dircache_mode() == 3 { DP_MAGIC } else { DC_MAGIC });
        out.bytes(&(dirs_out.len() as u64).to_le_bytes());
        for (p, s, l) in &dirs_out {
          out.path(p);
          out.stamp(s);
          out.bytes(&(l.len() as u32).to_le_bytes());
          for (name, kind) in l.iter() {
            let b = os_str_bytes(name);
            out.bytes(&[*kind]);
            out.bytes(&(b.len() as u16).to_le_bytes());
            out.bytes(&b);
          }
        }
        write_atomic(dir_persist_path().unwrap(), &out.0);
      }
    });
    *SAVING.lock().unwrap() = Some(h);
  }
  Some(line)
}

// ---------------------------------------------------------------------------------------------

fn union_ns(v: &mut [(u64, u64)]) -> (u64, u64, u64) {
  if v.is_empty() {
    return (0, 0, 0);
  }
  v.sort_unstable();
  let (mut total, mut cur_s, mut cur_e) = (0u64, v[0].0, v[0].1);
  for &(s, e) in v.iter().skip(1) {
    if s > cur_e {
      total += cur_e - cur_s;
      cur_s = s;
      cur_e = e;
    } else if e > cur_e {
      cur_e = e;
    }
  }
  total += cur_e - cur_s;
  let first = v[0].0;
  let last = v.iter().map(|x| x.1).max().unwrap();
  (total, first, last)
}

pub fn reset() {
  let _ = epoch();
  for s in ALL {
    s.reset();
  }
  LOAD_INTERVALS.lock().unwrap().clear();
  ALL_INTERVALS.lock().unwrap().clear();
  warm_start_build();
}

/// `ROLLDOWN_FS_TIMES=1`: print only the build/scan wall times (+ prefetch stats).
pub fn times_on() -> bool {
  env_is("ROLLDOWN_FS_TIMES", "1") || stats_on()
}

pub fn report(label: &str, scan_start: Instant, scan_end: Instant, total_end: Instant) {
  dump();
  let cc_line = warm_save();
  if !times_on() {
    return;
  }
  if let Some(l) = cc_line {
    eprintln!("{l}");
  }
  let ms = |n: u64| n as f64 / 1e6;
  let scan_ns = scan_end.duration_since(scan_start).as_nanos() as u64;
  let total_ns = total_end.duration_since(scan_start).as_nanos() as u64;
  eprintln!("=== fs profile [{label}] ===");
  eprintln!("build total {:.1} ms, scan stage {:.1} ms", ms(total_ns), ms(scan_ns));
  prefetch_report(scan_start);
  if !stats_on() {
    return;
  }
  eprintln!("{:<30} {:>8} {:>12} {:>10} {:>10}", "op", "count", "sum ms", "avg us", "MB");
  for s in ALL {
    let c = s.count.load(Relaxed);
    let n = s.nanos.load(Relaxed);
    let b = s.bytes.load(Relaxed);
    if c == 0 {
      continue;
    }
    eprintln!(
      "{:<30} {:>8} {:>12.1} {:>10.1} {:>10.2}",
      s.name,
      c,
      ms(n),
      n as f64 / c as f64 / 1e3,
      b as f64 / 1e6
    );
  }
  let scan_s = ns(scan_start);
  for (name, iv) in [("load reads", &LOAD_INTERVALS), ("all fs ops", &ALL_INTERVALS)] {
    let mut v = iv.lock().unwrap().clone();
    let (u, first, last) = union_ns(&mut v);
    if v.is_empty() {
      continue;
    }
    eprintln!(
      "{name}: wall time with >=1 op in flight {:.1} ms; window {:.1}..{:.1} ms after scan start; mean in-flight {:.2}",
      ms(u),
      ms(first.saturating_sub(scan_s)),
      ms(last.saturating_sub(scan_s)),
      v.iter().map(|x| x.1 - x.0).sum::<u64>() as f64 / u.max(1) as f64
    );
  }
}
