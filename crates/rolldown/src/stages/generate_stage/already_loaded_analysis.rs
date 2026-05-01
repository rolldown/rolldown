use std::collections::VecDeque;

use rolldown_common::{EntryPoint, ModuleIdx};
use rolldown_utils::{BitSet, IndexBitSet, indexmap::FxIndexMap};
use rustc_hash::FxHashMap;

use super::code_splitting::IndexSplittingInfo;

/// Pre-grouping pass that strips dynamic-entry bits from modules guaranteed
/// to already be in memory whenever the dynamic entry runs.
///
/// Mirrors Rollup's `getAlreadyLoadedAtomsByEntry` (`chunkAssignment.ts`).
/// Operates at module granularity rather than atom granularity — modules
/// with identical bits will coalesce naturally during the existing
/// bits-bucketing pass in `split_chunks`.
///
/// Awaited vs non-awaited dynamic imports are not distinguished; rolldown
/// does not track await context in `import_record`. This matches Rollup's
/// non-awaited branch only.
///
/// `runtime_module_idx` is excluded from stripping. The runtime module has
/// dedicated placement logic (`rehome_runtime_module`) that depends on its
/// bitset reflecting every chunk that consumes its helpers. Mutating those
/// bits prematurely would cause cycles — runtime moves into a single-bit
/// chunk while other modules sharing the dynamic-entry bit cross-import it.
pub fn propagate_already_loaded_atoms(
  entries: &FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
  index_splitting_info: &mut IndexSplittingInfo,
  entries_len: u32,
  runtime_module_idx: Option<ModuleIdx>,
) {
  if entries_len == 0 {
    return;
  }

  let module_count = index_splitting_info.len();
  let ctx = AnalysisContext::build(
    entries,
    index_splitting_info,
    entries_len,
    module_count,
    runtime_module_idx,
  );

  if ctx.dynamic_importers_by_dynamic_entry.is_empty() {
    return;
  }

  let already_loaded = ctx.run_fixed_point(module_count);
  ctx.apply_strip(index_splitting_info, &already_loaded);
}

struct AnalysisContext {
  entries_len: u32,
  /// Each entry's own ModuleIdx, indexed by entry_index.
  entry_module: Vec<ModuleIdx>,
  /// Bit set for entry indices that correspond to `EntryPointKind::DynamicImport`.
  is_dynamic_entry: BitSet,
  /// For each dynamic entry D, the entry indices that statically reach an
  /// importer of D (i.e. entries that dynamically depend on D).
  dynamic_importers_by_dynamic_entry: FxHashMap<u32, BitSet>,
  /// For each entry E, the dynamic entries it dynamically imports. Used as
  /// the worklist re-enqueue list when E's `already_loaded` shrinks.
  dynamic_imports_by_entry: Vec<BitSet>,
  /// For each entry E, the modules statically reachable from E (the modules
  /// whose `bits` contain E).
  static_modules_by_entry: Vec<IndexBitSet<ModuleIdx>>,
  /// Module excluded from stripping (the runtime module; see
  /// [`propagate_already_loaded_atoms`] for rationale).
  excluded_module: Option<ModuleIdx>,
}

impl AnalysisContext {
  fn build(
    entries: &FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
    index_splitting_info: &IndexSplittingInfo,
    entries_len: u32,
    module_count: usize,
    excluded_module: Option<ModuleIdx>,
  ) -> Self {
    let entries_len_usize = entries_len as usize;
    let mut entry_module = Vec::with_capacity(entries_len_usize);
    let mut is_dynamic_entry = BitSet::new(entries_len);

    for (entry_index, (&module_idx, entry_point)) in
      entries.iter().flat_map(|(idx, entries)| entries.iter().map(move |e| (idx, e))).enumerate()
    {
      debug_assert_eq!(entry_index, entry_module.len(), "entry_index must be sequential");
      entry_module.push(module_idx);
      if entry_point.kind.is_dynamic_import() {
        let bit = u32::try_from(entry_index).expect("Too many entries, u32 overflowed.");
        is_dynamic_entry.set_bit(bit);
      }
    }

    let mut static_modules_by_entry: Vec<IndexBitSet<ModuleIdx>> =
      (0..entries_len_usize).map(|_| IndexBitSet::new(module_count)).collect();
    for (module_idx, info) in index_splitting_info.iter_enumerated() {
      for entry_idx in info.bits.index_of_one() {
        static_modules_by_entry[entry_idx as usize].set_bit(module_idx);
      }
    }

    let mut dynamic_importers_by_dynamic_entry: FxHashMap<u32, BitSet> = FxHashMap::default();
    let mut dynamic_imports_by_entry: Vec<BitSet> =
      (0..entries_len_usize).map(|_| BitSet::new(entries_len)).collect();

    for (entry_index, (_, entry_point)) in
      entries.iter().flat_map(|(idx, entries)| entries.iter().map(move |e| (idx, e))).enumerate()
    {
      if !entry_point.kind.is_dynamic_import() {
        continue;
      }
      let dyn_idx = u32::try_from(entry_index).expect("Too many entries, u32 overflowed.");

      // For each importer module, every entry whose bit is set in the
      // importer's reachability bits is a dynamic dependent of `dyn_idx`.
      let mut importers = BitSet::new(entries_len);
      for &(importer_module_idx, _, _, _) in &entry_point.related_stmt_infos {
        let importer_bits = &index_splitting_info[importer_module_idx].bits;
        importers.union(importer_bits);
      }

      // Strip the dynamic entry's own bit from its importers — D is not its
      // own importer (and may have been transitively reached statically by
      // the importer module's bits in pathological inputs).
      importers.clear_bit(dyn_idx);

      if importers.is_empty() {
        continue;
      }

      for importer_entry_idx in importers.index_of_one() {
        dynamic_imports_by_entry[importer_entry_idx as usize].set_bit(dyn_idx);
      }
      dynamic_importers_by_dynamic_entry.insert(dyn_idx, importers);
    }

    Self {
      entries_len,
      entry_module,
      is_dynamic_entry,
      dynamic_importers_by_dynamic_entry,
      dynamic_imports_by_entry,
      static_modules_by_entry,
      excluded_module,
    }
  }

  fn run_fixed_point(&self, module_count: usize) -> Vec<IndexBitSet<ModuleIdx>> {
    let entries_len_usize = self.entries_len as usize;

    // Initialize the dynamic-entry seed as the union of all per-entry static
    // module sets. Modules unreachable from any entry can never satisfy the
    // already-loaded predicate, so they are safely excluded.
    let all_modules_full: IndexBitSet<ModuleIdx> = {
      let mut bits = IndexBitSet::new(module_count);
      for set in &self.static_modules_by_entry {
        bits.union(set);
      }
      bits
    };

    let mut already_loaded: Vec<IndexBitSet<ModuleIdx>> = (0..entries_len_usize)
      .map(|i| {
        if self.is_dynamic_entry.has_bit(u32::try_from(i).expect("entry_index overflow")) {
          all_modules_full.clone()
        } else {
          IndexBitSet::new(module_count)
        }
      })
      .collect();

    let mut queue: VecDeque<u32> = VecDeque::new();
    let mut in_queue = BitSet::new(self.entries_len);
    for &d in self.dynamic_importers_by_dynamic_entry.keys() {
      queue.push_back(d);
      in_queue.set_bit(d);
    }

    while let Some(d) = queue.pop_front() {
      in_queue.clear_bit(d);
      let importers = &self.dynamic_importers_by_dynamic_entry[&d];

      let mut new_loaded = already_loaded[d as usize].clone();
      for e in importers.index_of_one() {
        let mut union = self.static_modules_by_entry[e as usize].clone();
        union.union(&already_loaded[e as usize]);
        new_loaded.intersect_with(&union);
      }

      if new_loaded != already_loaded[d as usize] {
        already_loaded[d as usize] = new_loaded;
        for d_next in self.dynamic_imports_by_entry[d as usize].index_of_one() {
          if !in_queue.has_bit(d_next) {
            queue.push_back(d_next);
            in_queue.set_bit(d_next);
          }
        }
      }
    }

    already_loaded
  }

  fn apply_strip(
    &self,
    index_splitting_info: &mut IndexSplittingInfo,
    already_loaded: &[IndexBitSet<ModuleIdx>],
  ) {
    for (m_idx, info) in index_splitting_info.iter_mut_enumerated() {
      if info.bits.bit_count() <= 1 {
        continue;
      }
      if Some(m_idx) == self.excluded_module {
        continue;
      }
      let candidates: Vec<u32> = info.bits.index_of_one().collect();
      for e in candidates {
        if !self.is_dynamic_entry.has_bit(e) {
          continue;
        }
        if self.entry_module[e as usize] == m_idx {
          continue;
        }
        if !already_loaded[e as usize].has_bit(m_idx) {
          continue;
        }
        if info.bits.bit_count() <= 1 {
          break;
        }
        info.bits.clear_bit(e);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use arcstr::ArcStr;
  use oxc::allocator::Address;
  use oxc_index::IndexVec;
  use rolldown_common::{
    EntryPoint, EntryPointKind, ImportRecordIdx, ModuleIdx, ModuleTagBitSet, StmtInfoIdx,
  };
  use rolldown_utils::{BitSet, indexmap::FxIndexMap};

  use super::super::code_splitting::SplittingInfo;
  use super::propagate_already_loaded_atoms;

  fn mk_module(idx: usize) -> ModuleIdx {
    ModuleIdx::from_raw(u32::try_from(idx).expect("test idx fits in u32"))
  }

  fn mk_info(bits: &[u32], entries_len: u32) -> SplittingInfo {
    let mut bs = BitSet::new(entries_len);
    for &b in bits {
      bs.set_bit(b);
    }
    SplittingInfo { bits: bs, share_count: 0, tags_bit_set: ModuleTagBitSet::default() }
  }

  fn mk_entry(idx: ModuleIdx, kind: EntryPointKind, importers: &[ModuleIdx]) -> EntryPoint {
    EntryPoint {
      name: Some(ArcStr::from("e")),
      idx,
      kind,
      file_name: None,
      related_stmt_infos: importers
        .iter()
        .map(|&m| (m, StmtInfoIdx::from_raw(0), Address::DUMMY, ImportRecordIdx::from_raw(0)))
        .collect(),
    }
  }

  /// `A -> B; A => C; C -> B` — B's bit C should be stripped.
  /// Entry indices: 0=A (UserDefined), 1=C (DynamicImport).
  /// Modules: 0=A, 1=B, 2=C.
  /// B starts with bits {0, 1}; should end with bits {0}.
  /// C starts with bits {1}; should remain {1}.
  /// A starts with bits {0}; should remain {0}.
  #[test]
  fn basic_dynamic_already_loaded() {
    let a = mk_module(0);
    let b = mk_module(1);
    let c = mk_module(2);

    let mut entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>> = FxIndexMap::default();
    entries.insert(a, vec![mk_entry(a, EntryPointKind::UserDefined, &[])]);
    // C is dynamically imported by module A.
    entries.insert(c, vec![mk_entry(c, EntryPointKind::DynamicImport, &[a])]);

    let entries_len = 2u32;
    let mut info: IndexVec<ModuleIdx, SplittingInfo> = IndexVec::from_iter([
      mk_info(&[0], entries_len),
      mk_info(&[0, 1], entries_len),
      mk_info(&[1], entries_len),
    ]);

    propagate_already_loaded_atoms(&entries, &mut info, entries_len, None);

    assert!(info[a].bits.has_bit(0) && !info[a].bits.has_bit(1));
    assert!(
      info[b].bits.has_bit(0) && !info[b].bits.has_bit(1),
      "B's dynamic bit should be stripped"
    );
    assert!(
      !info[c].bits.has_bit(0) && info[c].bits.has_bit(1),
      "C should retain its own entry bit"
    );
  }

  /// Two static entries A1, A2 both import D dynamically; D imports M.
  /// M is statically reachable from A1 and A2 (bits {0, 1}). D's bit (2)
  /// should be stripped from M, since D's already_loaded includes M
  /// (intersection of A1's static modules and A2's static modules contains M).
  #[test]
  fn intersection_two_importers() {
    let a1 = mk_module(0);
    let a2 = mk_module(1);
    let d = mk_module(2);
    let m = mk_module(3);

    let mut entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>> = FxIndexMap::default();
    entries.insert(a1, vec![mk_entry(a1, EntryPointKind::UserDefined, &[])]);
    entries.insert(a2, vec![mk_entry(a2, EntryPointKind::UserDefined, &[])]);
    // D is dynamically imported by both A1 (via a1's module) and A2 (via a2's module).
    entries.insert(d, vec![mk_entry(d, EntryPointKind::DynamicImport, &[a1, a2])]);

    let entries_len = 3u32;
    let mut info: IndexVec<ModuleIdx, SplittingInfo> = IndexVec::from_iter([
      mk_info(&[0], entries_len),       // A1
      mk_info(&[1], entries_len),       // A2
      mk_info(&[2], entries_len),       // D
      mk_info(&[0, 1, 2], entries_len), // M
    ]);

    propagate_already_loaded_atoms(&entries, &mut info, entries_len, None);

    assert!(info[m].bits.has_bit(0));
    assert!(info[m].bits.has_bit(1));
    assert!(!info[m].bits.has_bit(2), "D's bit should be stripped from M");
  }

  /// `A -> M; A => D; D -> M`. M has bits {0, 1}.
  /// Single static importer A; M is in A's static set; D's already_loaded
  /// contains M; D's bit should be stripped from M.
  /// The dynamic entry module D itself has bits {1}; the self-entry guard
  /// must prevent D's bit from being stripped from D.
  #[test]
  fn self_entry_guard() {
    let a = mk_module(0);
    let d = mk_module(1);
    let m = mk_module(2);

    let mut entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>> = FxIndexMap::default();
    entries.insert(a, vec![mk_entry(a, EntryPointKind::UserDefined, &[])]);
    entries.insert(d, vec![mk_entry(d, EntryPointKind::DynamicImport, &[a])]);

    let entries_len = 2u32;
    let mut info: IndexVec<ModuleIdx, SplittingInfo> = IndexVec::from_iter([
      mk_info(&[0], entries_len),    // A
      mk_info(&[0, 1], entries_len), // D — both bits because A statically reaches D's import site
      mk_info(&[0, 1], entries_len), // M
    ]);

    propagate_already_loaded_atoms(&entries, &mut info, entries_len, None);

    assert!(info[m].bits.has_bit(0) && !info[m].bits.has_bit(1), "M's dynamic bit stripped");
    assert!(info[d].bits.has_bit(1), "D's own entry bit must not be stripped from D");
  }

  /// `A => B; B => A` mutual dynamic imports. The fixed point must
  /// terminate; modules unique to each entry retain their own bit.
  #[test]
  fn mutual_dynamic_cycle_terminates() {
    let a = mk_module(0);
    let b = mk_module(1);

    let mut entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>> = FxIndexMap::default();
    // Both are dynamic entries that import each other.
    entries.insert(a, vec![mk_entry(a, EntryPointKind::DynamicImport, &[b])]);
    entries.insert(b, vec![mk_entry(b, EntryPointKind::DynamicImport, &[a])]);

    let entries_len = 2u32;
    let mut info: IndexVec<ModuleIdx, SplittingInfo> =
      IndexVec::from_iter([mk_info(&[0], entries_len), mk_info(&[1], entries_len)]);

    // Should not hang or panic.
    propagate_already_loaded_atoms(&entries, &mut info, entries_len, None);

    assert!(info[a].bits.has_bit(0));
    assert!(info[b].bits.has_bit(1));
  }
}
