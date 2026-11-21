use std::{ffi::OsString, path::Path, sync::Arc};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  Key,
  format::{Footer, ListGroup},
};

pub type SegmentId = u32;

/// Orders segments: a newer segment is greater.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rank {
  created_ms: u64,
  segment: SegmentId,
}

pub struct Segment {
  pub name: OsString,
  pub path: Arc<Path>,
  pub created_ms: u64,
  pub file_len: u64,
  /// All values in the file, also the ones that are expired or have a newer copy elsewhere.
  pub value_bytes: u64,
  /// The latest time of the marks in the file. 0 when it has none.
  pub last_mark: u64,
}

pub struct Entries {
  pub results: Vec<((Key, Key), Location)>,
  /// Lists of each group are oldest first.
  pub groups: Vec<ListGroup>,
}

#[derive(Default, Clone, Copy)]
pub struct SegmentUsage {
  /// True when the segment holds any data that is not expired, including marks.
  pub live: bool,
  pub live_value_bytes: u64,
}

#[derive(Clone, Copy)]
pub struct Location {
  pub segment: SegmentId,
  rank: Rank,
  pub offset: u64,
  pub len: u64,
  pub checksum: u64,
  pub last_used: u64,
}

impl Location {
  pub fn same_place(&self, other: &Self) -> bool {
    self.segment == other.segment && self.offset == other.offset
  }
}

pub struct ListRecord {
  pub paths: Arc<[String]>,
  pub segment: SegmentId,
  /// Newer lists are greater. The second part is the position inside the segment.
  order: (Rank, u32),
}

#[derive(Default)]
pub struct KeyLists {
  /// Newest first.
  pub records: Vec<ListRecord>,
  pub last_used: u64,
}

/// What this process knows about the segments in the cache dir.
#[derive(Default)]
pub struct Index {
  /// Names of segment files already handled: loaded, rejected, or merged by this process.
  pub seen: FxHashSet<OsString>,
  /// Segment files that could not be read: damaged, or in another format.
  pub rejected: FxHashSet<OsString>,
  pub segments: FxHashMap<SegmentId, Segment>,
  pub results: FxHashMap<(Key, Key), Location>,
  pub lists: FxHashMap<Key, KeyLists>,
  next_segment: SegmentId,
}

impl Index {
  /// Adds the lists and results of a segment. Marks are applied by `apply_marks`, after all
  /// segments of a batch are added, so that a mark finds its entry.
  pub fn add_segment(&mut self, name: OsString, path: Arc<Path>, file_len: u64, footer: &Footer) {
    let id = self.next_segment;
    self.next_segment += 1;
    self.segments.insert(
      id,
      Segment {
        name,
        path,
        created_ms: footer.created_ms,
        file_len,
        value_bytes: footer.results.iter().map(|rec| rec.len).sum(),
        last_mark: footer
          .list_marks
          .iter()
          .map(|(_, time)| *time)
          .chain(footer.result_marks.iter().map(|(_, time)| *time))
          .max()
          .unwrap_or(0),
      },
    );
    let rank = Rank { created_ms: footer.created_ms, segment: id };

    for group in &footer.groups {
      self.add_group(group, id, rank);
    }
    for rec in &footer.results {
      let loc = Location {
        segment: id,
        rank,
        offset: rec.offset,
        len: rec.len,
        checksum: rec.checksum,
        last_used: rec.last_used,
      };
      self
        .results
        .entry((rec.key1, rec.key2))
        .and_modify(|old| {
          let last_used = old.last_used.max(loc.last_used);
          if loc.rank > old.rank {
            *old = loc;
          }
          old.last_used = last_used;
        })
        .or_insert(loc);
    }
  }

  fn add_group(&mut self, group: &ListGroup, segment: SegmentId, rank: Rank) {
    let key_lists = self.lists.entry(group.key1).or_default();
    key_lists.last_used = key_lists.last_used.max(group.last_used);
    for (pos, paths) in (0u32..).zip(&group.lists) {
      let order = (rank, pos);
      if let Some(old) = key_lists.records.iter_mut().find(|r| r.paths == *paths) {
        if order > old.order {
          old.order = order;
          old.segment = segment;
        }
      } else {
        key_lists.records.push(ListRecord { paths: Arc::clone(paths), segment, order });
      }
    }
    key_lists.records.sort_unstable_by_key(|r| std::cmp::Reverse(r.order));
  }

  pub fn apply_marks(&mut self, footer: &Footer) {
    for (key1, time) in &footer.list_marks {
      if let Some(key_lists) = self.lists.get_mut(key1) {
        key_lists.last_used = key_lists.last_used.max(*time);
      }
    }
    for (key, time) in &footer.result_marks {
      if let Some(loc) = self.results.get_mut(key) {
        loc.last_used = loc.last_used.max(*time);
      }
    }
  }

  /// Oldest first.
  pub fn segments_by_age(&self) -> Vec<SegmentId> {
    let mut ids: Vec<SegmentId> = self.segments.keys().copied().collect();
    ids.sort_unstable_by_key(|id| Rank { created_ms: self.segments[id].created_ms, segment: *id });
    ids
  }

  /// The entries whose newest copy is in `ids` and that are used at or after `cutoff`.
  pub fn entries_in(&self, ids: &FxHashSet<SegmentId>, cutoff: Option<u64>) -> Entries {
    let live_key1s = cutoff.map(|c| self.live_key1s(c));
    let results = self
      .results
      .iter()
      .filter(|(_, loc)| ids.contains(&loc.segment) && cutoff.is_none_or(|c| loc.last_used >= c))
      .map(|(key, loc)| (*key, *loc))
      .collect();
    let groups = self
      .lists
      .iter()
      .filter(|(key1, _)| live_key1s.as_ref().is_none_or(|live| live.contains(*key1)))
      .filter_map(|(&key1, key_lists)| {
        let lists: Vec<Arc<[String]>> = key_lists
          .records
          .iter()
          .rev()
          .filter(|r| ids.contains(&r.segment))
          .map(|r| Arc::clone(&r.paths))
          .collect();
        (!lists.is_empty()).then_some(ListGroup { key1, last_used: key_lists.last_used, lists })
      })
      .collect();
    Entries { results, groups }
  }

  pub fn segment_usage(&self, cutoff: u64) -> FxHashMap<SegmentId, SegmentUsage> {
    let mut usage: FxHashMap<SegmentId, SegmentUsage> = FxHashMap::default();
    for loc in self.results.values().filter(|loc| loc.last_used >= cutoff) {
      let segment = usage.entry(loc.segment).or_default();
      segment.live = true;
      segment.live_value_bytes += loc.len;
    }
    let live_key1s = self.live_key1s(cutoff);
    for (_, key_lists) in self.lists.iter().filter(|(key1, _)| live_key1s.contains(*key1)) {
      for record in &key_lists.records {
        usage.entry(record.segment).or_default().live = true;
      }
    }
    for (id, segment) in &self.segments {
      if segment.last_mark >= cutoff {
        usage.entry(*id).or_default().live = true;
      }
    }
    usage
  }

  /// Forgets the segments and every entry stored in them.
  pub fn remove_segments(&mut self, ids: &FxHashSet<SegmentId>) {
    if ids.is_empty() {
      return;
    }
    self.segments.retain(|id, _| !ids.contains(id));
    self.results.retain(|_, loc| !ids.contains(&loc.segment));
    self.lists.retain(|_, key_lists| {
      key_lists.records.retain(|r| !ids.contains(&r.segment));
      !key_lists.records.is_empty()
    });
  }

  /// Keys whose lists stay when entries older than `cutoff` are removed.
  ///
  /// The rule for lists: the lists of a key 1 stay while the key 1 has any data used at or after
  /// `cutoff`. That is a list of it was written or read, or one of its results was used.
  pub fn live_key1s(&self, cutoff: u64) -> FxHashSet<Key> {
    let mut live: FxHashSet<Key> = self
      .results
      .iter()
      .filter(|(_, loc)| loc.last_used >= cutoff)
      .map(|((key1, _), _)| *key1)
      .collect();
    live.extend(self.lists.iter().filter(|(_, kl)| kl.last_used >= cutoff).map(|(key1, _)| *key1));
    live
  }
}
