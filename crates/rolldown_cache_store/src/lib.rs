//! On-disk store of the persistent scan cache.
//!
//! The store keeps immutable segment files in one directory. Each flush adds one new segment.
//! Several stores, in one process or in several processes, can use one directory at the same
//! time without a lock, because they only add files and treat missing or damaged data as not
//! found. See `format.rs` for the file format.

mod file;
mod format;
mod index;
#[cfg(test)]
mod tests;

use std::{
  ffi::OsString,
  fs::{self, File},
  io,
  path::{Path, PathBuf},
  sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard},
  time::Duration,
};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  file::SegmentWriter,
  format::{Footer, ListGroup, ListMark, ResultMark, ResultRecord},
  index::{Entries, Index, SegmentId},
};

pub type Key = u128;

type LoadedSegment = (OsString, Arc<Path>, Option<(Footer, u64)>);

/// When there are more small segments than `MAX_SMALL_SEGMENTS`, they are merged. Large
/// segments are never merged, so one merge copies at most about 16 * 4 MiB, however large the
/// cache is.
const MAX_SMALL_SEGMENTS: usize = 16;
const SMALL_SEGMENT_BYTES: u64 = 4 * 1024 * 1024;
/// Temp files older than this come from interrupted writes.
const TEMP_FILE_MAX_AGE_MS: u64 = 60 * 60 * 1000;

pub struct StoreOptions {
  pub dir: PathBuf,
  /// Entries not used for longer than this are removed. `None` removes nothing.
  pub max_age: Option<Duration>,
}

pub struct CacheStore {
  dir: PathBuf,
  max_age_ms: Option<u64>,
  index: RwLock<Index>,
  /// Lock order: never take `pending` while holding `index`.
  pending: Mutex<Pending>,
  flush_lock: Mutex<()>,
  #[cfg(test)]
  test_now_ms: std::sync::atomic::AtomicU64,
}

impl std::fmt::Debug for CacheStore {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CacheStore").field("dir", &self.dir).finish_non_exhaustive()
  }
}

#[derive(Default)]
struct Pending {
  current: Batch,
  /// The batch that a flush is writing. It stays visible until the new segment is in `index`.
  flushing: Option<Arc<Batch>>,
}

#[derive(Default, Clone)]
struct Batch {
  /// Oldest first.
  lists: FxHashMap<Key, Vec<Arc<[String]>>>,
  results: FxHashMap<(Key, Key), Vec<u8>>,
  list_marks: FxHashSet<Key>,
  result_marks: FxHashSet<(Key, Key)>,
}

impl Batch {
  fn is_empty(&self) -> bool {
    self.lists.is_empty()
      && self.results.is_empty()
      && self.list_marks.is_empty()
      && self.result_marks.is_empty()
  }

  fn has_list(&self, key1: Key, list: &[String]) -> bool {
    self.lists.get(&key1).is_some_and(|lists| lists.iter().any(|l| **l == *list))
  }
}

impl Pending {
  fn batches(&self) -> impl Iterator<Item = &Batch> {
    std::iter::once(&self.current).chain(self.flushing.as_deref())
  }
}

impl CacheStore {
  /// Never fails. A missing or unreadable dir gives an empty store. Does not create the dir.
  pub fn open(options: StoreOptions) -> Self {
    let store = Self {
      dir: options.dir,
      max_age_ms: options.max_age.map(|age| u64::try_from(age.as_millis()).unwrap_or(u64::MAX)),
      index: RwLock::default(),
      pending: Mutex::default(),
      flush_lock: Mutex::default(),
      #[cfg(test)]
      test_now_ms: std::sync::atomic::AtomicU64::new(0),
    };
    store.refresh();
    store
  }

  /// Loads segments that appeared in the dir since open/last refresh (written by other
  /// processes or by this store), and forgets segments that were removed.
  pub fn refresh(&self) {
    let Ok(names) = file::list_segment_names(&self.dir) else { return };

    let (new_names, maybe_gone) = {
      let index = self.read_index();
      let new_names: Vec<OsString> =
        names.iter().filter(|name| !index.seen.contains(*name)).cloned().collect();
      let maybe_gone: Vec<OsString> =
        index.seen.iter().filter(|name| !names.contains(*name)).cloned().collect();
      (new_names, maybe_gone)
    };
    if new_names.is_empty() && maybe_gone.is_empty() {
      return;
    }

    // The listing can be older than a segment this store has just written, so check that a
    // file is really gone before forgetting it. A removed name never comes back.
    let gone: FxHashSet<OsString> = maybe_gone
      .into_iter()
      .filter(|name| {
        matches!(fs::symlink_metadata(self.dir.join(name)), Err(err) if err.kind() == io::ErrorKind::NotFound)
      })
      .collect();

    let mut loaded: Vec<LoadedSegment> = new_names
      .into_iter()
      .map(|name| {
        let path: Arc<Path> = self.dir.join(&name).into();
        let footer = file::read_footer(&path);
        (name, path, footer)
      })
      .collect();
    loaded.sort_by(|a, b| {
      let a_ms = a.2.as_ref().map(|(f, _)| f.created_ms);
      let b_ms = b.2.as_ref().map(|(f, _)| f.created_ms);
      a_ms.cmp(&b_ms).then_with(|| a.0.cmp(&b.0))
    });

    let mut index = self.write_index();
    let mut added = Vec::new();
    for (name, path, footer) in loaded {
      if !index.seen.insert(name.clone()) {
        continue;
      }
      match footer {
        Some((footer, file_len)) => {
          index.add_segment(name, path, file_len, &footer);
          added.push(footer);
        }
        None => {
          index.rejected.insert(name);
        }
      }
    }
    for footer in &added {
      index.apply_marks(footer);
    }
    if !gone.is_empty() {
      let ids: FxHashSet<SegmentId> = index
        .segments
        .iter()
        .filter(|(_, segment)| gone.contains(&segment.name))
        .map(|(id, _)| *id)
        .collect();
      index.remove_segments(&ids);
      index.seen.retain(|name| !gone.contains(name));
      index.rejected.retain(|name| !gone.contains(name));
    }
  }

  /// Dependency lists saved under key1, newest first. Includes entries put() in this process.
  pub fn dependency_lists(&self, key1: Key) -> Vec<Arc<[String]>> {
    let mut out = Vec::new();
    for batch in self.lock_pending().batches() {
      if let Some(lists) = batch.lists.get(&key1) {
        out.extend(lists.iter().rev().map(Arc::clone));
      }
    }
    let found_on_disk = {
      let index = self.read_index();
      index.lists.get(&key1).is_some_and(|key_lists| {
        out.extend(key_lists.records.iter().map(|r| Arc::clone(&r.paths)));
        true
      })
    };
    if found_on_disk && self.max_age_ms.is_some() {
      self.lock_pending().current.list_marks.insert(key1);
    }
    out
  }

  /// The value saved under (key1, key2). None if absent, damaged, or its file was removed.
  /// Checks the checksum.
  pub fn get(&self, key1: Key, key2: Key) -> Option<Vec<u8>> {
    let key = (key1, key2);
    {
      let pending = self.lock_pending();
      if let Some(value) = pending.batches().find_map(|batch| batch.results.get(&key)) {
        return Some(value.clone());
      }
    }
    for attempt in 0..2 {
      let (loc, path) = {
        let index = self.read_index();
        let loc = *index.results.get(&key)?;
        (loc, Arc::clone(&index.segments.get(&loc.segment)?.path))
      };
      let read = File::open(&path)
        .and_then(|file| file::read_value(&file, loc.offset, loc.len, loc.checksum));
      match read {
        Ok(Some(value)) => {
          if self.max_age_ms.is_some() {
            self.lock_pending().current.result_marks.insert(key);
          }
          return Some(value);
        }
        // Another process may have merged the segment into a new one and removed it.
        Err(err) if err.kind() == io::ErrorKind::NotFound && attempt == 0 => self.refresh(),
        Ok(None) | Err(_) => {
          // Forget the damaged entry, so that a later put() can save it again.
          let mut index = self.write_index();
          if index.results.get(&key).is_some_and(|current| current.same_place(&loc)) {
            index.results.remove(&key);
          }
          return None;
        }
      }
    }
    None
  }

  /// Adds an entry. Visible to dependency_lists/get in this process at once. Written by the next
  /// flush. An identical dependency list under key1 is stored once; an existing (key1,key2) is
  /// not added again.
  pub fn put(&self, key1: Key, dependency_list: Arc<[String]>, key2: Key, value: Vec<u8>) {
    let key = (key1, key2);
    let track_usage = self.max_age_ms.is_some();
    let mut pending = self.lock_pending();
    let (list_on_disk, result_on_disk) = {
      let index = self.read_index();
      (
        index
          .lists
          .get(&key1)
          .is_some_and(|kl| kl.records.iter().any(|r| r.paths == dependency_list)),
        index.results.contains_key(&key),
      )
    };

    if list_on_disk {
      if track_usage {
        pending.current.list_marks.insert(key1);
      }
    } else if !pending.batches().any(|batch| batch.has_list(key1, &dependency_list)) {
      pending.current.lists.entry(key1).or_default().push(dependency_list);
    }

    if result_on_disk {
      if track_usage {
        pending.current.result_marks.insert(key);
      }
    } else if !pending.batches().any(|batch| batch.results.contains_key(&key)) {
      pending.current.results.insert(key, value);
    }
  }

  /// True if put() was called since the last flush (or there is usage to record).
  pub fn has_pending(&self) -> bool {
    !self.lock_pending().current.is_empty()
  }

  /// Writes all pending entries into one new segment file, then runs maintenance: merging
  /// small segments and removing data older than `max_age`. Creates the dir if needed.
  ///
  /// When the write fails, the pending entries are dropped and not saved.
  pub fn flush(&self) -> io::Result<()> {
    let _flush_guard = self.flush_lock.lock().unwrap_or_else(PoisonError::into_inner);
    let batch = {
      let mut pending = self.lock_pending();
      if pending.current.is_empty() {
        return Ok(());
      }
      let batch = Arc::new(std::mem::take(&mut pending.current));
      pending.flushing = Some(Arc::clone(&batch));
      batch
    };
    let now = self.now_ms();

    let written = self.write_batch(&batch, now);
    drop(batch);
    if let Ok((name, path, file_len, footer)) = &written {
      let mut index = self.write_index();
      if index.seen.insert(name.clone()) {
        index.add_segment(name.clone(), path.as_path().into(), *file_len, footer);
        index.apply_marks(footer);
      }
    }
    // On failure the batch is dropped too. Keeping it for the next flush would let memory grow
    // without limit in watch mode when the dir stays unwritable.
    self.lock_pending().flushing = None;
    written?;

    self.maintain(now)
  }

  fn write_batch(&self, batch: &Batch, now: u64) -> io::Result<(OsString, PathBuf, u64, Footer)> {
    let mut writer = SegmentWriter::create(&self.dir, now)?;
    let mut results = Vec::with_capacity(batch.results.len());
    for (&(key1, key2), value) in &batch.results {
      let (offset, checksum) = writer.write_value(value)?;
      results.push(ResultRecord {
        key1,
        key2,
        offset,
        len: value.len() as u64,
        checksum,
        last_used: now,
      });
    }
    let footer = Footer {
      created_ms: now,
      groups: batch
        .lists
        .iter()
        .map(|(&key1, lists)| ListGroup { key1, last_used: now, lists: lists.clone() })
        .collect(),
      results,
      list_marks: batch.list_marks.iter().map(|&key1| (key1, now)).collect(),
      result_marks: batch.result_marks.iter().map(|&key| (key, now)).collect(),
    };
    let (name, path, file_len) = writer.finish(&footer)?;
    Ok((name, path, file_len, footer))
  }

  fn maintain(&self, now: u64) -> io::Result<()> {
    self.refresh();
    file::remove_old_files(&self.dir, now.saturating_sub(TEMP_FILE_MAX_AGE_MS), file::is_temp_name);

    let cutoff = self.max_age_ms.map(|age| now.saturating_sub(age));
    if let Some(cutoff) = cutoff {
      let rejected = self.read_index().rejected.clone();
      if !rejected.is_empty() {
        // Files this version cannot read, for example from another format version. They are
        // removed by age like other data, because no reader of this version can use them.
        file::remove_old_files(&self.dir, cutoff, |name| {
          rejected.contains(std::ffi::OsStr::new(name))
        });
      }
      self.remove_expired(cutoff)?;
    }
    self.merge_small_segments(cutoff)
  }

  /// Removes segments that hold only expired data. Rewrites a segment when at least half of
  /// its value bytes are expired. Other expired entries stay until then.
  fn remove_expired(&self, cutoff: u64) -> io::Result<()> {
    let (expired, sparse) = {
      let index = self.read_index();
      let usage = index.segment_usage(cutoff);
      let mut expired = FxHashSet::default();
      let mut sparse = Vec::new();
      for (id, segment) in &index.segments {
        let usage = usage.get(id).copied().unwrap_or_default();
        let unused_bytes = segment.value_bytes.saturating_sub(usage.live_value_bytes);
        if !usage.live {
          expired.insert(*id);
        } else if segment.value_bytes > 0 && unused_bytes * 2 >= segment.value_bytes {
          sparse.push(*id);
        }
      }
      (expired, sparse)
    };
    self.remove_segments(&expired);
    for id in sparse {
      self.copy_segments(&[id], Some(cutoff))?;
    }
    Ok(())
  }

  fn merge_small_segments(&self, cutoff: Option<u64>) -> io::Result<()> {
    let runs = {
      let index = self.read_index();
      let by_age = index.segments_by_age();
      let is_small = |id: &SegmentId| index.segments[id].file_len < SMALL_SEGMENT_BYTES;
      if by_age.iter().filter(|id| is_small(id)).count() <= MAX_SMALL_SEGMENTS {
        return Ok(());
      }
      // Only neighbors in age order are merged together, so that the merged segment takes the
      // same place in the newest-first order as the segments it replaces.
      by_age
        .split(|id| !is_small(id))
        .filter(|run| run.len() > 1)
        .map(<[SegmentId]>::to_vec)
        .collect::<Vec<_>>()
    };
    for run in runs {
      self.copy_segments(&run, cutoff)?;
    }
    Ok(())
  }

  /// Writes the live entries of `ids` into one new segment and removes the old segments.
  ///
  /// The new segment gets the creation time of the newest old one, so the order of dependency
  /// lists stays the same when `ids` are neighbors in age order. Marks keep their own time.
  fn copy_segments(&self, ids: &[SegmentId], cutoff: Option<u64>) -> io::Result<()> {
    let ids: FxHashSet<SegmentId> = ids.iter().copied().collect();
    let (created_ms, paths, mark_paths, Entries { results: locations, groups }) = {
      let index = self.read_index();
      let segments: Vec<(SegmentId, &index::Segment)> =
        ids.iter().filter_map(|id| Some((*id, index.segments.get(id)?))).collect();
      let Some(created_ms) = segments.iter().map(|(_, s)| s.created_ms).max() else {
        return Ok(());
      };
      let paths: FxHashMap<SegmentId, Arc<Path>> =
        segments.iter().map(|(id, s)| (*id, Arc::clone(&s.path))).collect();
      let mark_paths: Vec<Arc<Path>> = segments
        .iter()
        .filter(|(_, s)| s.last_mark > 0 && cutoff.is_none_or(|c| s.last_mark >= c))
        .map(|(_, s)| Arc::clone(&s.path))
        .collect();
      (created_ms, paths, mark_paths, index.entries_in(&ids, cutoff))
    };
    let (list_marks, result_marks) = self.marks_to_keep(&mark_paths, &ids, &groups, cutoff);

    let mut written = None;
    if !(locations.is_empty()
      && groups.is_empty()
      && list_marks.is_empty()
      && result_marks.is_empty())
    {
      let mut writer = SegmentWriter::create(&self.dir, created_ms)?;
      let mut results = Vec::with_capacity(locations.len());
      let mut files: FxHashMap<SegmentId, Option<File>> = FxHashMap::default();
      for ((key1, key2), loc) in locations {
        let file = files
          .entry(loc.segment)
          .or_insert_with(|| paths.get(&loc.segment).and_then(|path| File::open(path).ok()));
        // A value that cannot be read is dropped. It is damaged, or another process removed
        // its segment after merging it.
        let Some(value) = file.as_ref().and_then(|file| {
          file::read_value(file, loc.offset, loc.len, loc.checksum).ok().flatten()
        }) else {
          continue;
        };
        let (offset, checksum) = writer.write_value(&value)?;
        results.push(ResultRecord {
          key1,
          key2,
          offset,
          len: loc.len,
          checksum,
          last_used: loc.last_used,
        });
      }
      drop(files);
      let footer = Footer { created_ms, groups, results, list_marks, result_marks };
      let (name, path, file_len) = writer.finish(&footer)?;
      written = Some((name, path, file_len, footer));
    }

    {
      let mut index = self.write_index();
      index.remove_segments(&ids);
      if let Some((name, path, file_len, footer)) = written {
        if index.seen.insert(name.clone()) {
          index.add_segment(name, path.into(), file_len, &footer);
        }
      }
    }
    remove_files(paths.values());
    Ok(())
  }

  /// Marks in the files of `ids` that still matter: not expired, their entry still exists, and
  /// their entry is not copied (a copied entry already has the time in its `last_used`).
  fn marks_to_keep(
    &self,
    paths: &[Arc<Path>],
    ids: &FxHashSet<SegmentId>,
    groups: &[ListGroup],
    cutoff: Option<u64>,
  ) -> (Vec<ListMark>, Vec<ResultMark>) {
    let mut list_marks: FxHashMap<Key, u64> = FxHashMap::default();
    let mut result_marks: FxHashMap<(Key, Key), u64> = FxHashMap::default();
    let live = |time: &u64| cutoff.is_none_or(|c| *time >= c);
    for path in paths {
      // A file that cannot be read loses its marks. Removing usage data is always safe.
      let Some((footer, _)) = file::read_footer(path) else { continue };
      for (key1, time) in footer.list_marks.into_iter().filter(|(_, time)| live(time)) {
        let kept = list_marks.entry(key1).or_default();
        *kept = (*kept).max(time);
      }
      for (key, time) in footer.result_marks.into_iter().filter(|(_, time)| live(time)) {
        let kept = result_marks.entry(key).or_default();
        *kept = (*kept).max(time);
      }
    }
    let copied_key1s: FxHashSet<Key> = groups.iter().map(|group| group.key1).collect();
    let index = self.read_index();
    list_marks.retain(|key1, _| index.lists.contains_key(key1) && !copied_key1s.contains(key1));
    result_marks
      .retain(|key, _| index.results.get(key).is_some_and(|loc| !ids.contains(&loc.segment)));
    (list_marks.into_iter().collect(), result_marks.into_iter().collect())
  }

  fn remove_segments(&self, ids: &FxHashSet<SegmentId>) {
    if ids.is_empty() {
      return;
    }
    let paths: Vec<Arc<Path>> = {
      let mut index = self.write_index();
      let paths =
        ids.iter().filter_map(|id| Some(Arc::clone(&index.segments.get(id)?.path))).collect();
      index.remove_segments(ids);
      paths
    };
    remove_files(&paths);
  }

  fn now_ms(&self) -> u64 {
    #[cfg(test)]
    {
      let now = self.test_now_ms.load(std::sync::atomic::Ordering::Relaxed);
      if now != 0 {
        return now;
      }
    }
    file::system_now_ms()
  }

  #[cfg(test)]
  fn set_now_ms(&self, now: u64) {
    self.test_now_ms.store(now, std::sync::atomic::Ordering::Relaxed);
  }

  fn read_index(&self) -> RwLockReadGuard<'_, Index> {
    self.index.read().unwrap_or_else(PoisonError::into_inner)
  }

  fn write_index(&self) -> RwLockWriteGuard<'_, Index> {
    self.index.write().unwrap_or_else(PoisonError::into_inner)
  }

  fn lock_pending(&self) -> MutexGuard<'_, Pending> {
    self.pending.lock().unwrap_or_else(PoisonError::into_inner)
  }
}

fn remove_files<'a>(paths: impl IntoIterator<Item = &'a Arc<Path>>) {
  for path in paths {
    // Can fail on Windows while another process reads the file, or when another process has
    // already removed it. Leaving the file only costs disk space.
    let _ = fs::remove_file(path);
  }
}
