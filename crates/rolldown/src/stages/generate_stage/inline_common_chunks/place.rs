use std::collections::{VecDeque, hash_map::Entry};

use oxc_index::IndexVec;
use petgraph::{algo::tarjan_scc, prelude::DiGraphMap};
use rolldown_common::ChunkIdx;
use rolldown_utils::{BitSet, indexmap::FxIndexSet};
use rustc_hash::{FxHashMap, FxHashSet};

/// Who reads and who carries each record, derived from a chunk -> importee edge table.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct InlinePlacement {
  /// File or record -> the records it reads directly (record execution order).
  pub readers: FxHashMap<ChunkIdx, Vec<ChunkIdx>>,
  /// File -> the records whose factories it prints, dependencies first: every record reachable
  /// from the records it reads along record -> record read edges, minus the records a file it
  /// imports after projection (a record importee stands for the record's own importees) prints
  /// or inherits outside the importer's import cycle, and records registered on every loading
  /// path from an entry before this file starts evaluating.
  pub carried: FxHashMap<ChunkIdx, Vec<ChunkIdx>>,
  /// The files that read at least one record, carrying or not, sorted by chunk index.
  pub reading_files: Vec<ChunkIdx>,
}

impl InlinePlacement {
  pub fn is_carried(&self, record: ChunkIdx) -> bool {
    self.carried.values().any(|carried| carried.contains(&record))
  }
}

/// `static_importees[c]` are the chunks `c` statically imports; `is_live` excludes removed
/// chunks; `exec_order` orders records the way the chunk graph sorts chunks.
pub(super) fn compute_placement(
  static_importees: &IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>,
  dynamic_importees: &IndexVec<ChunkIdx, FxIndexSet<ChunkIdx>>,
  untracked_dynamic_importers: &FxHashSet<ChunkIdx>,
  is_live: impl Fn(ChunkIdx) -> bool,
  is_entry: impl Fn(ChunkIdx) -> bool,
  exec_order: impl Fn(ChunkIdx) -> u32,
  records: &FxIndexSet<ChunkIdx>,
) -> InlinePlacement {
  let mut readers: FxHashMap<ChunkIdx, Vec<ChunkIdx>> = FxHashMap::default();
  for (chunk_idx, importees) in static_importees.iter_enumerated() {
    if !is_live(chunk_idx) {
      continue;
    }
    let mut read = importees
      .iter()
      .copied()
      .filter(|importee| *importee != chunk_idx && records.contains(importee))
      .collect::<Vec<_>>();
    if read.is_empty() {
      continue;
    }
    read.sort_unstable_by_key(|idx| (exec_order(*idx), idx.raw()));
    readers.insert(chunk_idx, read);
  }

  // A file needs the closure of what it reads: a factory it prints may require other records.
  let mut needed: FxHashMap<ChunkIdx, Vec<ChunkIdx>> = FxHashMap::default();
  for (chunk_idx, read) in &readers {
    if records.contains(chunk_idx) {
      continue;
    }
    let mut visited = FxHashSet::default();
    let mut order = Vec::new();
    for record in read {
      visit(*record, &readers, &mut visited, &mut order);
    }
    needed.insert(*chunk_idx, order);
  }

  // The file graph after projection: a record importee stands for the record's own importees,
  // records among them expanded the same way. `apply_inline_common_chunks_links` writes these
  // edges and ESM evaluates them, so a registration is inherited along them.
  let mut projected: FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>> = FxHashMap::default();
  for (chunk_idx, importees) in static_importees.iter_enumerated() {
    if !is_live(chunk_idx) || records.contains(&chunk_idx) {
      continue;
    }
    let mut files = FxHashSet::default();
    let mut expanded = FxHashSet::default();
    for importee in importees {
      expand(*importee, static_importees, &is_live, records, &mut expanded, &mut files);
    }
    files.remove(&chunk_idx);
    projected.insert(chunk_idx, files);
  }

  // Import cycles: inside one, which member runs first depends on the entry, so nothing is
  // inherited across its members. `tarjan_scc` lists components dependencies first.
  let mut graph = DiGraphMap::<ChunkIdx, ()>::new();
  for (chunk_idx, importees) in &projected {
    graph.add_node(*chunk_idx);
    for importee in importees {
      graph.add_edge(*chunk_idx, *importee, ());
    }
  }
  let components = tarjan_scc(&graph);
  let mut component_of: FxHashMap<ChunkIdx, usize> = FxHashMap::default();
  for (index, component) in components.iter().enumerate() {
    for chunk_idx in component {
      component_of.insert(*chunk_idx, index);
    }
  }

  // registered[f]: the records registered in file `f` or in a file `f` imports after projection
  // outside `f`'s cycle, i.e. registered before `f`'s body runs. Records register nothing
  // themselves: a factory holds module code, the getter table and its own bridges.
  let mut registered: FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>> = FxHashMap::default();
  let mut carried: FxHashMap<ChunkIdx, Vec<ChunkIdx>> = FxHashMap::default();
  for (index, component) in components.iter().enumerate() {
    let inherited = component
      .iter()
      .map(|file| {
        let mut set = FxHashSet::default();
        for dependency in &projected[file] {
          if component_of.get(dependency) == Some(&index) {
            continue;
          }
          if let Some(before) = registered.get(dependency) {
            set.extend(before.iter().copied());
          }
        }
        set
      })
      .collect::<Vec<_>>();
    for (file, inherited) in component.iter().zip(inherited) {
      let own: Vec<ChunkIdx> = needed
        .get(file)
        .map(|order| order.iter().copied().filter(|record| !inherited.contains(record)).collect())
        .unwrap_or_default();
      let mut set = inherited;
      set.extend(own.iter().copied());
      if !own.is_empty() {
        carried.insert(*file, own);
      }
      registered.insert(*file, set);
    }
  }

  prune_async_inherited_records(
    &projected,
    dynamic_importees,
    untracked_dynamic_importers,
    &is_entry,
    records,
    &registered,
    &mut carried,
  );

  let mut reading_files =
    readers.keys().copied().filter(|idx| !records.contains(idx)).collect::<Vec<_>>();
  reading_files.sort_unstable();
  InlinePlacement { readers, carried, reading_files }
}

// See internal-docs/inline-common-chunks/design.md#registration-on-entry-loading-paths.
fn prune_async_inherited_records(
  projected: &FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>>,
  dynamic_importees: &IndexVec<ChunkIdx, FxIndexSet<ChunkIdx>>,
  untracked_dynamic_importers: &FxHashSet<ChunkIdx>,
  is_entry: &impl Fn(ChunkIdx) -> bool,
  records: &FxIndexSet<ChunkIdx>,
  registered: &FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>>,
  carried: &mut FxHashMap<ChunkIdx, Vec<ChunkIdx>>,
) {
  if carried.is_empty()
    || (untracked_dynamic_importers.is_empty()
      && dynamic_importees.iter().all(FxIndexSet::is_empty))
  {
    return;
  }
  let record_count = u32::try_from(records.len()).expect("Too many inline records");
  let record_bits: FxHashMap<_, _> = records.iter().copied().zip(0..record_count).collect();
  // Every possible copy contributes its loader edges. Pruning copies can only remove edges,
  // so this table conservatively includes every parent of the emitted dynamic imports.
  let projected_dynamic: FxHashMap<_, _> = projected
    .keys()
    .map(|file| {
      let mut targets = dynamic_importees[*file].clone();
      for record in carried.get(file).into_iter().flatten() {
        targets.extend(dynamic_importees[*record].iter().copied());
      }
      (*file, targets)
    })
    .collect();

  let mut before = FxHashMap::<ChunkIdx, BitSet>::default();
  let mut pending = VecDeque::new();
  let mut queued = FxHashSet::default();
  for file in projected.keys().copied().filter(|file| is_entry(*file)) {
    before.insert(file, BitSet::new(record_count));
    pending.push_back(file);
    queued.insert(file);
  }
  while let Some(file) = pending.pop_front() {
    queued.remove(&file);
    let at_start = before[&file].clone();
    let mut at_dynamic_import = at_start.clone();
    at_dynamic_import.extend(registered[&file].iter().map(|record| record_bits[record]));
    // A static dependency evaluates before the importing file's registrations. A dynamic
    // import runs after those registrations and its dependencies outside the static cycle.
    let unknown_targets = untracked_dynamic_importers.contains(&file).then(|| projected.keys());
    for (&target, available) in projected[&file]
      .iter()
      .map(|target| (target, &at_start))
      .chain(projected_dynamic[&file].iter().map(|target| (target, &at_dynamic_import)))
      .chain(unknown_targets.into_iter().flatten().map(|target| (target, &at_dynamic_import)))
    {
      if !projected.contains_key(&target) {
        continue;
      }
      let changed = match before.entry(target) {
        Entry::Vacant(entry) => {
          entry.insert(available.clone());
          true
        }
        Entry::Occupied(mut entry) => {
          let mut intersection = entry.get().clone();
          intersection.intersect(available);
          if intersection == *entry.get() {
            false
          } else {
            entry.insert(intersection);
            true
          }
        }
      };
      if changed && queued.insert(target) {
        pending.push_back(target);
      }
    }
  }
  carried.retain(|file, own| {
    if let Some(available) = before.get(file) {
      own.retain(|record| !available.has_bit(record_bits[record]));
    }
    !own.is_empty()
  });
}

/// Adds `importee` to `files`, or, when it is a record, the files behind it: its importees,
/// records among them expanded the same way, each record once.
fn expand(
  importee: ChunkIdx,
  static_importees: &IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>,
  is_live: &impl Fn(ChunkIdx) -> bool,
  records: &FxIndexSet<ChunkIdx>,
  expanded: &mut FxHashSet<ChunkIdx>,
  files: &mut FxHashSet<ChunkIdx>,
) {
  if !is_live(importee) {
    return;
  }
  if !records.contains(&importee) {
    files.insert(importee);
    return;
  }
  if !expanded.insert(importee) {
    return;
  }
  for next in &static_importees[importee] {
    expand(*next, static_importees, is_live, records, expanded, files);
  }
}

fn visit(
  record: ChunkIdx,
  readers: &FxHashMap<ChunkIdx, Vec<ChunkIdx>>,
  visited: &mut FxHashSet<ChunkIdx>,
  order: &mut Vec<ChunkIdx>,
) {
  if !visited.insert(record) {
    return;
  }
  for dependency in readers.get(&record).into_iter().flatten() {
    visit(*dependency, readers, visited, order);
  }
  order.push(record);
}

#[cfg(test)]
mod tests {
  use super::*;

  fn idx(raw: u32) -> ChunkIdx {
    ChunkIdx::from_raw(raw)
  }

  /// `edges[i]` lists what chunk `i` imports; every chunk is live and executes in index order.
  fn place(edges: &[&[u32]], records: &[u32]) -> InlinePlacement {
    let dynamic = vec![&[][..]; edges.len()];
    let entries = (0..u32::try_from(edges.len()).unwrap()).collect::<Vec<_>>();
    place_with_imports(edges, &dynamic, &entries, records)
  }

  fn place_with_imports(
    edges: &[&[u32]],
    dynamic: &[&[u32]],
    entries: &[u32],
    records: &[u32],
  ) -> InlinePlacement {
    let static_importees: IndexVec<ChunkIdx, FxHashSet<ChunkIdx>> =
      edges.iter().map(|importees| importees.iter().map(|i| idx(*i)).collect()).collect();
    let dynamic_importees =
      dynamic.iter().map(|importees| importees.iter().map(|i| idx(*i)).collect()).collect();
    let records = records.iter().map(|i| idx(*i)).collect();
    compute_placement(
      &static_importees,
      &dynamic_importees,
      &FxHashSet::default(),
      |_| true,
      |chunk_idx| entries.contains(&chunk_idx.raw()),
      ChunkIdx::raw,
      &records,
    )
  }

  fn list(items: &[u32]) -> Vec<ChunkIdx> {
    items.iter().map(|i| idx(*i)).collect()
  }

  #[test]
  fn every_reader_carries_when_nothing_is_inherited() {
    // 0 and 1 are entries, 2 is the record both read.
    let placement = place(&[&[2], &[2], &[]], &[2]);
    assert_eq!(placement.reading_files, list(&[0, 1]));
    assert_eq!(placement.carried[&idx(0)], list(&[2]));
    assert_eq!(placement.carried[&idx(1)], list(&[2]));
  }

  #[test]
  fn a_file_carries_the_closure_of_what_it_reads_dependencies_first() {
    // entry 0 reads record 1, which reads record 2, which reads record 3.
    let placement = place(&[&[1], &[2], &[3], &[]], &[1, 2, 3]);
    assert_eq!(placement.carried[&idx(0)], list(&[3, 2, 1]));
    assert_eq!(placement.readers[&idx(1)], list(&[2]));
    assert_eq!(placement.reading_files, list(&[0]));
  }

  #[test]
  fn a_static_dependency_outside_the_cycle_registers_for_its_importers() {
    // entry 0 imports file 1; both read record 2; 1 is not in a cycle with 0.
    let placement = place(&[&[1, 2], &[2], &[]], &[2]);
    assert_eq!(placement.carried[&idx(1)], list(&[2]));
    assert!(!placement.carried.contains_key(&idx(0)), "0 inherits the registration from 1");
    assert_eq!(placement.reading_files, list(&[0, 1]), "0 still reads the record");
  }

  #[test]
  fn nothing_is_inherited_inside_an_import_cycle() {
    // files 0 and 1 import each other and both read record 2.
    let placement = place(&[&[1, 2], &[0, 2], &[]], &[2]);
    assert_eq!(placement.carried[&idx(0)], list(&[2]));
    assert_eq!(placement.carried[&idx(1)], list(&[2]));
  }

  #[test]
  fn a_registration_behind_a_record_reaches_the_reader_through_projection() {
    // entry 0 reads records 1 and 3; record 1 imports file 2, which reads record 3. After
    // projection 0 imports 2, which registers 3 before 0 runs, so 0 carries only 1.
    let placement = place(&[&[1, 3], &[2], &[3], &[]], &[1, 3]);
    assert_eq!(placement.carried[&idx(0)], list(&[1]));
    assert_eq!(placement.carried[&idx(2)], list(&[3]));
    assert_eq!(placement.readers[&idx(0)], list(&[1, 3]));
  }

  #[test]
  fn a_reader_inherits_through_the_dependency_of_a_record_it_reads() {
    // entries 0, 1, 2; records 3 (r) and 4 (s); file 5 (v). 0 reads r and s, 1 reads r, 2 reads
    // s, r imports v, v reads s. Projection makes 0 and 1 import v, which carries s.
    let placement = place(&[&[3, 4], &[3], &[4], &[5], &[], &[4]], &[3, 4]);
    assert_eq!(placement.carried[&idx(0)], list(&[3]));
    assert_eq!(placement.carried[&idx(1)], list(&[3]));
    assert_eq!(placement.carried[&idx(2)], list(&[4]));
    assert_eq!(placement.carried[&idx(5)], list(&[4]));
  }

  #[test]
  fn a_cycle_closed_through_a_record_s_dependency_inherits_nothing() {
    // files 0 and 1: 0 reads record 2, which imports 1; 1 imports 0 and reads record 3. After
    // projection 0 and 1 import each other, so neither inherits from the other.
    let placement = place(&[&[2], &[0, 3], &[1], &[]], &[2, 3]);
    assert_eq!(placement.carried[&idx(0)], list(&[2]));
    assert_eq!(placement.carried[&idx(1)], list(&[3]));
  }

  #[test]
  fn inheritance_is_transitive_along_files() {
    // 0 -> 1 -> 2 (files), 2 reads record 3; 0 also reads 3 directly.
    let placement = place(&[&[1, 3], &[2], &[3], &[]], &[3]);
    assert_eq!(placement.carried[&idx(2)], list(&[3]));
    assert!(!placement.carried.contains_key(&idx(0)));
    assert!(!placement.carried.contains_key(&idx(1)));
  }

  #[test]
  fn a_record_no_file_reaches_is_not_carried() {
    // record 1 is read only by record 2, which nobody reads.
    let placement = place(&[&[], &[], &[1]], &[1, 2]);
    assert!(!placement.is_carried(idx(1)));
    assert!(!placement.is_carried(idx(2)));
  }

  #[test]
  fn removed_chunks_are_ignored() {
    let static_importees: IndexVec<ChunkIdx, FxHashSet<ChunkIdx>> =
      [vec![idx(2)], vec![idx(2)], vec![]].into_iter().map(FxHashSet::from_iter).collect();
    let records = FxIndexSet::from_iter([idx(2)]);
    let placement = compute_placement(
      &static_importees,
      &IndexVec::from_vec(vec![FxIndexSet::default(); 3]),
      &FxHashSet::default(),
      |chunk_idx| chunk_idx != idx(1),
      |_| true,
      ChunkIdx::raw,
      &records,
    );
    assert_eq!(placement.reading_files, list(&[0]));
  }

  #[test]
  fn every_dynamic_parent_supplies_the_record() {
    let placement =
      place_with_imports(&[&[3], &[3], &[3], &[]], &[&[2], &[2], &[], &[]], &[0, 1], &[3]);
    assert_eq!(placement.carried[&idx(0)], list(&[3]));
    assert_eq!(placement.carried[&idx(1)], list(&[3]));
    assert!(!placement.carried.contains_key(&idx(2)));
  }

  #[test]
  fn a_dynamic_parent_without_the_record_keeps_the_copy() {
    let placement =
      place_with_imports(&[&[3], &[], &[3], &[]], &[&[2], &[2], &[], &[]], &[0, 1], &[3]);
    assert_eq!(placement.carried[&idx(2)], list(&[3]));
  }

  #[test]
  fn an_entry_reached_dynamically_still_carries_its_record() {
    let placement = place_with_imports(&[&[2], &[2], &[]], &[&[1], &[], &[]], &[0, 1], &[2]);
    assert_eq!(placement.carried[&idx(1)], list(&[2]));
  }

  #[test]
  fn a_lazy_static_dependency_inherits_the_parent_registration() {
    let placement =
      place_with_imports(&[&[3], &[2], &[3], &[]], &[&[1], &[], &[], &[]], &[0], &[3]);
    assert_eq!(placement.carried[&idx(0)], list(&[3]));
    assert!(!placement.carried.contains_key(&idx(2)));
  }

  #[test]
  fn a_static_load_path_cannot_inherit_its_importers_registration() {
    let placement =
      place_with_imports(&[&[2, 3], &[3], &[3], &[]], &[&[1], &[], &[], &[]], &[0], &[3]);
    assert_eq!(placement.carried[&idx(2)], list(&[3]));
    assert!(!placement.carried.contains_key(&idx(1)));
  }

  #[test]
  fn nested_dynamic_cycles_inherit_only_from_reachable_entries() {
    let placement =
      place_with_imports(&[&[3], &[3], &[3], &[]], &[&[1], &[2], &[1], &[]], &[0], &[3]);
    assert_eq!(placement.carried[&idx(0)], list(&[3]));
    assert!(!placement.carried.contains_key(&idx(1)));
    assert!(!placement.carried.contains_key(&idx(2)));
  }

  #[test]
  fn every_copy_of_a_dynamic_loader_contributes_a_parent() {
    let placement = place_with_imports(
      &[&[3, 4], &[4], &[3], &[], &[]],
      &[&[], &[], &[], &[], &[2]],
      &[0, 1],
      &[3, 4],
    );
    assert_eq!(placement.carried[&idx(2)], list(&[3]));
  }
}
