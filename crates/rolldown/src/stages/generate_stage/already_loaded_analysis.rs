use std::collections::VecDeque;

use oxc_index::IndexVec;
use rolldown_common::{ChunkIdx, EntryPoint, ModuleIdx};
use rolldown_utils::{BitSet, IndexBitSet, indexmap::FxIndexMap};
use rustc_hash::{FxHashMap, FxHashSet};

use super::code_splitting::IndexSplittingInfo;
use crate::types::linking_metadata::LinkingMetadataVec;

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
///
/// `strict_signature_entries` is a bitset (over entry indices) marking
/// user-defined static entries whose `preserveEntrySignatures` resolves to
/// `Strict`. A strip is suppressed if it would leave the module with a
/// single remaining bit pointing at one of these entries — that path
/// bypasses the strict-signature gate inside
/// `try_insert_into_existing_chunk` (which only fires when a *common*
/// chunk is about to be merged into a strict entry). Keeping the module
/// multi-bit routes it through the common-chunk path, where the existing
/// `can_merge_without_changing_entry_signature` check decides whether the
/// merge is safe.
///
/// `metas` provides static dependency information for the post-strip cycle
/// check: after stripping, the projected chunk-dependency graph is built
/// and any modules whose strips contributed to a cycle are reverted to
/// their pre-strip bits. Pre-strip bits are assumed to give an acyclic
/// chunk graph, so reverting cycle-participants restores acyclicity.
///
/// `module_to_chunk` provides pre-assignment information from manual code
/// splitting: any module already pinned to a chunk (e.g. by a
/// `manualCodeSplitting` rule) is projected into that chunk during cycle
/// detection rather than bits-bucketed alongside other modules sharing the
/// same `bits` value. Without this, manually-extracted chunks (which
/// remain in their own chunk regardless of bits) are invisible to the
/// projection and the resulting cycle goes undetected (issue #9225).
pub fn propagate_already_loaded_atoms(
  entries: &FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
  index_splitting_info: &mut IndexSplittingInfo,
  entries_len: u32,
  runtime_module_idx: Option<ModuleIdx>,
  strict_signature_entries: &BitSet,
  metas: &LinkingMetadataVec,
  module_to_chunk: &IndexVec<ModuleIdx, Option<ChunkIdx>>,
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
    strict_signature_entries.clone(),
  );

  if ctx.dynamic_importers_by_dynamic_entry.is_empty() {
    return;
  }

  let already_loaded = ctx.run_fixed_point(module_count);

  // Snapshot pre-strip bits so we can roll back cycle participants.
  let pre_strip_bits: IndexVec<ModuleIdx, BitSet> =
    index_splitting_info.iter().map(|info| info.bits.clone()).collect();

  ctx.apply_strip(index_splitting_info, &already_loaded);

  revert_cycle_participants(index_splitting_info, &pre_strip_bits, metas, module_to_chunk);
}

/// Project the post-strip chunk graph and revert strips on any module
/// whose projected chunk participates in a static-import cycle.
///
/// Algorithm:
/// 1. Group modules by post-strip `bits` to obtain projected chunk IDs.
/// 2. Build a chunk-level forward edge set from `metas[m].dependencies`
///    (which contains static deps only — `ImportKind::DynamicImport` and
///    `ImportKind::Require` are excluded at construction in
///    `link_stage/mod.rs`).
/// 3. Find SCC members larger than one node *or* nodes with self-edges
///    (cycle participants).
/// 4. For each module mapped to a cycle chunk whose bits actually changed
///    during the strip, restore its pre-strip bits.
///
/// Pre-strip bits give an acyclic graph by assumption (the input to the
/// pass came out of static reachability + manual splitting), so reverting
/// the diff for cycle participants is sufficient. Iteration is bounded —
/// each pass strictly reduces the number of strips that remain in effect.
fn revert_cycle_participants(
  index_splitting_info: &mut IndexSplittingInfo,
  pre_strip_bits: &IndexVec<ModuleIdx, BitSet>,
  metas: &LinkingMetadataVec,
  module_to_chunk: &IndexVec<ModuleIdx, Option<ChunkIdx>>,
) {
  // Hard cap on revert iterations. Termination is monotonic — each pass
  // strictly reduces the number of strips that remain in effect, so the
  // count is bounded by the number of modules in the worst case. The cap
  // is a defense against pathological inputs.
  const MAX_ITERATIONS: u32 = 32;

  for _ in 0..MAX_ITERATIONS {
    if !revert_cycle_participants_once(
      index_splitting_info,
      pre_strip_bits,
      metas,
      module_to_chunk,
    ) {
      return;
    }
  }
  // If we hit the cap, the projected graph still has cycles. Roll back all
  // remaining strips as a last resort so the bucketing pass starts from an
  // acyclic state.
  for (m_idx, info) in index_splitting_info.iter_mut_enumerated() {
    let pre = &pre_strip_bits[m_idx];
    if info.bits != *pre {
      info.bits = pre.clone();
    }
  }
}

/// One revert pass. Returns `true` if any module's bits were rolled back,
/// meaning the projected chunk graph may have changed and the caller
/// should re-check.
fn revert_cycle_participants_once(
  index_splitting_info: &mut IndexSplittingInfo,
  pre_strip_bits: &IndexVec<ModuleIdx, BitSet>,
  metas: &LinkingMetadataVec,
  module_to_chunk: &IndexVec<ModuleIdx, Option<ChunkIdx>>,
) -> bool {
  // (1) Project chunks. Pre-assigned modules (manual code splitting) keep
  // their pinned chunk identity; everything else is grouped by post-strip
  // bits. The two namespaces are kept disjoint via `ProjectionKey`.
  #[derive(PartialEq, Eq, Hash, Clone)]
  enum ProjectionKey {
    Pinned(ChunkIdx),
    Bits(BitSet),
  }

  let mut chunk_ids: FxHashMap<ProjectionKey, u32> = FxHashMap::default();
  let mut module_chunk: IndexVec<ModuleIdx, Option<u32>> =
    IndexVec::from_iter(std::iter::repeat_n(None, index_splitting_info.len()));
  for (m_idx, info) in index_splitting_info.iter_enumerated() {
    let key = if let Some(chunk_idx) = module_to_chunk.get(m_idx).copied().flatten() {
      ProjectionKey::Pinned(chunk_idx)
    } else if !info.bits.is_empty() {
      ProjectionKey::Bits(info.bits.clone())
    } else {
      continue;
    };
    let next = u32::try_from(chunk_ids.len()).expect("chunk count fits in u32");
    let id = *chunk_ids.entry(key).or_insert(next);
    module_chunk[m_idx] = Some(id);
  }
  let n_chunks = chunk_ids.len();
  if n_chunks == 0 {
    return false;
  }

  // (2) Build forward adjacency between chunks via static dependencies.
  // Use `Vec<Vec<u32>>` so Tarjan's SCC can iterate neighbors directly
  // without per-node allocations. A `FxHashSet` deduplicates first to
  // collapse parallel edges, then we materialize the final adjacency
  // lists once.
  let mut edge_sets: Vec<FxHashSet<u32>> = vec![FxHashSet::default(); n_chunks];
  for (m_idx, _info) in index_splitting_info.iter_enumerated() {
    let Some(my_chunk) = module_chunk[m_idx] else { continue };
    for &dep_idx in &metas[m_idx].dependencies {
      let Some(dep_chunk) = module_chunk[dep_idx] else { continue };
      if dep_chunk != my_chunk {
        edge_sets[my_chunk as usize].insert(dep_chunk);
      }
    }
  }
  let adjacency: Vec<Vec<u32>> = edge_sets.into_iter().map(|s| s.into_iter().collect()).collect();

  // (3) Find cycle participants via Tarjan's SCC. Any SCC of size > 1
  // *or* a single node with a self-edge is a cycle.
  let cycle_chunks = find_cycle_chunks(&adjacency);
  if cycle_chunks.is_empty() {
    return false;
  }

  // (4) Revert strips for modules in cycle chunks.
  let mut reverted_any = false;
  for (m_idx, info) in index_splitting_info.iter_mut_enumerated() {
    let Some(c) = module_chunk[m_idx] else { continue };
    if !cycle_chunks.contains(&c) {
      continue;
    }
    let pre = &pre_strip_bits[m_idx];
    if info.bits != *pre {
      info.bits = pre.clone();
      reverted_any = true;
    }
  }

  reverted_any
}

/// Tarjan's SCC, returning the set of chunk IDs that participate in a
/// non-trivial cycle (SCC size > 1, or a single-node SCC with a self-edge).
///
/// Adjacency is precomputed by the caller (`Vec<Vec<u32>>`) so the inner
/// loop iterates neighbors by index without per-node allocation.
fn find_cycle_chunks(adjacency: &[Vec<u32>]) -> FxHashSet<u32> {
  let n = adjacency.len();
  let mut index_counter: u32 = 0;
  let mut stack: Vec<u32> = Vec::new();
  let mut on_stack: Vec<bool> = vec![false; n];
  let mut indices: Vec<i32> = vec![-1; n];
  let mut lowlinks: Vec<u32> = vec![0; n];
  let mut result: FxHashSet<u32> = FxHashSet::default();

  // Iterative DFS to avoid blowing the stack on large graphs.
  // Each frame: (node, neighbor_iter_pos).
  for start in 0..n {
    if indices[start] != -1 {
      continue;
    }
    let mut work: Vec<(u32, usize)> = Vec::new();
    let start_u = u32::try_from(start).expect("chunk count fits in u32");
    indices[start] = index_counter as i32;
    lowlinks[start] = index_counter;
    index_counter += 1;
    stack.push(start_u);
    on_stack[start] = true;
    work.push((start_u, 0));

    while let Some(&(node, edge_idx)) = work.last() {
      let neighbors = &adjacency[node as usize];
      if edge_idx < neighbors.len() {
        work.last_mut().unwrap().1 += 1;
        let v = neighbors[edge_idx];
        if indices[v as usize] == -1 {
          indices[v as usize] = index_counter as i32;
          lowlinks[v as usize] = index_counter;
          index_counter += 1;
          stack.push(v);
          on_stack[v as usize] = true;
          work.push((v, 0));
        } else if on_stack[v as usize] {
          let v_idx = indices[v as usize] as u32;
          if v_idx < lowlinks[node as usize] {
            lowlinks[node as usize] = v_idx;
          }
        }
      } else {
        // All neighbors processed. Maybe pop SCC.
        if lowlinks[node as usize] as i32 == indices[node as usize] {
          let mut scc: Vec<u32> = Vec::new();
          while let Some(top) = stack.pop() {
            on_stack[top as usize] = false;
            scc.push(top);
            if top == node {
              break;
            }
          }
          let is_cycle = scc.len() > 1
            || scc.iter().any(|&s| adjacency[s as usize].contains(&s));
          if is_cycle {
            for s in scc {
              result.insert(s);
            }
          }
        }
        work.pop();
        // Propagate lowlink up the stack.
        if let Some(&(parent, _)) = work.last() {
          let child_low = lowlinks[node as usize];
          if child_low < lowlinks[parent as usize] {
            lowlinks[parent as usize] = child_low;
          }
        }
      }
    }
  }

  result
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
  /// Entry indices whose preserve_entry_signature resolves to `Strict`.
  /// A strip is suppressed if it would leave the module with a single bit
  /// pointing at one of these entries (see [`propagate_already_loaded_atoms`]).
  strict_signature_entries: BitSet,
}

impl AnalysisContext {
  fn build(
    entries: &FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
    index_splitting_info: &IndexSplittingInfo,
    entries_len: u32,
    module_count: usize,
    excluded_module: Option<ModuleIdx>,
    strict_signature_entries: BitSet,
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
      strict_signature_entries,
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
        // Strict-signature gate: if this strip would leave the module with a
        // single remaining bit pointing at a strict-signature entry, the
        // module would land directly in that entry's chunk via the bucketing
        // pass — bypassing `try_insert_into_existing_chunk`'s strict check.
        // Keep the bit so the module flows through the common-chunk path
        // where the existing signature gate runs.
        if info.bits.bit_count() == 2 {
          let remaining = info.bits.index_of_one().find(|&b| b != e);
          if let Some(b) = remaining
            && self.strict_signature_entries.has_bit(b)
          {
            continue;
          }
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
  use crate::types::linking_metadata::{LinkingMetadata, LinkingMetadataVec};

  fn empty_metas(n: usize) -> LinkingMetadataVec {
    IndexVec::from_iter((0..n).map(|_| LinkingMetadata::default()))
  }

  fn empty_module_to_chunk(n: usize) -> IndexVec<ModuleIdx, Option<rolldown_common::ChunkIdx>> {
    IndexVec::from_iter(std::iter::repeat_n(None, n))
  }

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

    let module_count = info.len();
    propagate_already_loaded_atoms(
      &entries,
      &mut info,
      entries_len,
      None,
      &BitSet::new(entries_len),
      &empty_metas(module_count),
      &empty_module_to_chunk(module_count),
    );

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

    let module_count = info.len();
    propagate_already_loaded_atoms(
      &entries,
      &mut info,
      entries_len,
      None,
      &BitSet::new(entries_len),
      &empty_metas(module_count),
      &empty_module_to_chunk(module_count),
    );

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

    let module_count = info.len();
    propagate_already_loaded_atoms(
      &entries,
      &mut info,
      entries_len,
      None,
      &BitSet::new(entries_len),
      &empty_metas(module_count),
      &empty_module_to_chunk(module_count),
    );

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
    let module_count = info.len();
    propagate_already_loaded_atoms(
      &entries,
      &mut info,
      entries_len,
      None,
      &BitSet::new(entries_len),
      &empty_metas(module_count),
      &empty_module_to_chunk(module_count),
    );

    assert!(info[a].bits.has_bit(0));
    assert!(info[b].bits.has_bit(1));
  }

  /// Same shape as `basic_dynamic_already_loaded` (`A -> B; A => C; C -> B`)
  /// but A is marked strict-signature. Stripping C's bit from B would leave
  /// B with a single bit pointing at the strict entry A, bypassing the
  /// strict-signature gate inside `try_insert_into_existing_chunk`. The
  /// strip must therefore be suppressed and B kept multi-bit.
  #[test]
  fn strict_signature_gate_blocks_terminal_strip() {
    let a = mk_module(0);
    let b = mk_module(1);
    let c = mk_module(2);

    let mut entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>> = FxIndexMap::default();
    entries.insert(a, vec![mk_entry(a, EntryPointKind::UserDefined, &[])]);
    entries.insert(c, vec![mk_entry(c, EntryPointKind::DynamicImport, &[a])]);

    let entries_len = 2u32;
    let mut info: IndexVec<ModuleIdx, SplittingInfo> = IndexVec::from_iter([
      mk_info(&[0], entries_len),
      mk_info(&[0, 1], entries_len),
      mk_info(&[1], entries_len),
    ]);

    let mut strict = BitSet::new(entries_len);
    strict.set_bit(0);

    let module_count = info.len();
    propagate_already_loaded_atoms(
      &entries,
      &mut info,
      entries_len,
      None,
      &strict,
      &empty_metas(module_count),
      &empty_module_to_chunk(module_count),
    );

    assert!(
      info[b].bits.has_bit(0) && info[b].bits.has_bit(1),
      "B's dynamic bit must be retained when stripping would land it in a strict entry"
    );
  }

  /// Cycle revert: when stripping would create a static-import cycle in
  /// the projected chunk graph, the strip is reverted post-hoc.
  ///
  /// Shape (mirrors issue #9225's static-chain-through-manual-chunk):
  ///   entries: 0=main (UserDefined), 1=ext (UserDefined), 2=dyn (DynamicImport from main).
  ///   modules: main {0}, ext {1}, mid {0,1}, leaf {0,2}, dyn {2}.
  ///   static deps: main → mid, main → leaf, ext → mid, mid → leaf, dyn → leaf.
  ///
  /// The strip pass would clear leaf's bit 2 (leaf is statically reachable
  /// from main, the only importer of dyn). That moves leaf into chunk-{0},
  /// alongside main. Projected edges:
  ///   chunk-{0}   → chunk-{0,1}    (main → mid)
  ///   chunk-{0,1} → chunk-{0}      (mid → leaf, post-strip)
  ///
  /// That's a cycle. The revert must restore leaf.bits to its pre-strip
  /// value {0, 2} so leaf flows through a separate common chunk.
  #[test]
  fn cycle_revert_breaks_cycle() {
    let main = mk_module(0);
    let ext = mk_module(1);
    let mid = mk_module(2);
    let leaf = mk_module(3);
    let dyn_mod = mk_module(4);

    let mut entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>> = FxIndexMap::default();
    entries.insert(main, vec![mk_entry(main, EntryPointKind::UserDefined, &[])]);
    entries.insert(ext, vec![mk_entry(ext, EntryPointKind::UserDefined, &[])]);
    entries.insert(dyn_mod, vec![mk_entry(dyn_mod, EntryPointKind::DynamicImport, &[main])]);

    let entries_len = 3u32;
    let mut info: IndexVec<ModuleIdx, SplittingInfo> = IndexVec::from_iter([
      mk_info(&[0], entries_len),    // main
      mk_info(&[1], entries_len),    // ext
      mk_info(&[0, 1], entries_len), // mid — reached by main and ext
      mk_info(&[0, 2], entries_len), // leaf — reached by main and dyn (NOT ext)
      mk_info(&[2], entries_len),    // dyn
    ]);

    let module_count = info.len();
    let mut metas = empty_metas(module_count);
    metas[main].dependencies.insert(mid);
    metas[main].dependencies.insert(leaf);
    metas[ext].dependencies.insert(mid);
    metas[mid].dependencies.insert(leaf);
    metas[dyn_mod].dependencies.insert(leaf);

    propagate_already_loaded_atoms(
      &entries,
      &mut info,
      entries_len,
      None,
      &BitSet::new(entries_len),
      &metas,
      &empty_module_to_chunk(module_count),
    );

    // Without the revert, leaf would land in chunk-{0} and `mid → leaf`
    // would close a chunk-{0} ↔ chunk-{0,1} cycle. With the revert,
    // leaf's bit 2 must be restored.
    assert!(
      info[leaf].bits.has_bit(0) && info[leaf].bits.has_bit(2),
      "leaf's strip must be reverted to break the projected chunk cycle"
    );
  }

  /// Cycle revert with a manually-pinned chunk: the cycle only becomes
  /// visible to the projection because `module_to_chunk` reports the
  /// pinned module as a separate chunk identity, even though its bits
  /// would otherwise group it with main.
  ///
  /// Shape (mirrors the manual-chunk + dynamic-import case from #9225):
  ///   entries: 0=main (UserDefined), 1=dyn (DynamicImport, importer=main).
  ///   modules: main {0}, manual {0}, shared {0,1}, dyn {1}.
  ///   static deps: main → manual, manual → shared, dyn → shared.
  ///   manual is pinned to a synthetic ChunkIdx via `module_to_chunk`,
  ///   simulating extraction by `manualCodeSplitting`.
  ///
  /// Without `module_to_chunk` plumbing, manual's bits {0} would group it
  /// with main in chunk-{0}, making `main → manual` an intra-chunk edge
  /// and the cycle invisible. With the pinning, `main → manual` is a
  /// cross-chunk forward edge (chunk-{0} → manual-pin), and the
  /// post-strip `manual → shared` becomes a back edge (manual-pin →
  /// chunk-{0}). The revert pass must catch this and restore shared's
  /// pre-strip bits.
  #[test]
  fn cycle_revert_uses_manual_pinning() {
    use rolldown_common::ChunkIdx;

    let main = mk_module(0);
    let manual = mk_module(1);
    let shared = mk_module(2);
    let dyn_mod = mk_module(3);

    let mut entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>> = FxIndexMap::default();
    entries.insert(main, vec![mk_entry(main, EntryPointKind::UserDefined, &[])]);
    entries.insert(dyn_mod, vec![mk_entry(dyn_mod, EntryPointKind::DynamicImport, &[main])]);

    let entries_len = 2u32;
    let mut info: IndexVec<ModuleIdx, SplittingInfo> = IndexVec::from_iter([
      mk_info(&[0], entries_len),    // main
      mk_info(&[0], entries_len),    // manual — same bits as main; only pinning separates it
      mk_info(&[0, 1], entries_len), // shared — strippable
      mk_info(&[1], entries_len),    // dyn
    ]);

    let module_count = info.len();
    let mut metas = empty_metas(module_count);
    metas[main].dependencies.insert(manual);
    metas[manual].dependencies.insert(shared);
    metas[dyn_mod].dependencies.insert(shared);

    // Pin `manual` into its own (synthetic) chunk to simulate the
    // post-`apply_manual_code_splitting` state.
    let mut module_to_chunk = empty_module_to_chunk(module_count);
    module_to_chunk[manual] = Some(ChunkIdx::from_raw(99));

    propagate_already_loaded_atoms(
      &entries,
      &mut info,
      entries_len,
      None,
      &BitSet::new(entries_len),
      &metas,
      &module_to_chunk,
    );

    // Sanity: without manual pinning the projection collapses main and
    // manual into the same chunk, so the cycle is invisible and shared's
    // strip would not be reverted. Verify pinning catches the cycle.
    assert!(
      info[shared].bits.has_bit(0) && info[shared].bits.has_bit(1),
      "shared's strip must be reverted via the manual-pinned projection path"
    );
  }

  /// With three dynamic siblings stripping into a strict entry, the gate
  /// should suppress only the *last* strip — the earlier strips still leave
  /// `bit_count() >= 2`, so the module flows through the common-chunk path
  /// rather than landing directly in the strict entry's chunk.
  #[test]
  fn strict_signature_gate_only_blocks_final_strip() {
    let a = mk_module(0);
    let d1 = mk_module(1);
    let d2 = mk_module(2);
    let m = mk_module(3);

    let mut entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>> = FxIndexMap::default();
    entries.insert(a, vec![mk_entry(a, EntryPointKind::UserDefined, &[])]);
    entries.insert(d1, vec![mk_entry(d1, EntryPointKind::DynamicImport, &[a])]);
    entries.insert(d2, vec![mk_entry(d2, EntryPointKind::DynamicImport, &[a])]);

    let entries_len = 3u32;
    let mut info: IndexVec<ModuleIdx, SplittingInfo> = IndexVec::from_iter([
      mk_info(&[0], entries_len),       // A
      mk_info(&[1], entries_len),       // D1
      mk_info(&[2], entries_len),       // D2
      mk_info(&[0, 1, 2], entries_len), // M reachable from all three entries
    ]);

    let mut strict = BitSet::new(entries_len);
    strict.set_bit(0);

    let module_count = info.len();
    propagate_already_loaded_atoms(
      &entries,
      &mut info,
      entries_len,
      None,
      &strict,
      &empty_metas(module_count),
      &empty_module_to_chunk(module_count),
    );

    // Bit 1 (D1) is the first strippable candidate; stripping it leaves
    // {0, 2} with bit_count == 2, so the gate does not fire.
    // Bit 2 (D2) would then leave {0} alone — the gate fires and bit 2 is
    // kept. Final state: {0, 2}, multi-bit, routed via common chunk.
    assert!(info[m].bits.has_bit(0), "static entry bit retained");
    assert!(!info[m].bits.has_bit(1), "first dynamic bit stripped (non-terminal)");
    assert!(info[m].bits.has_bit(2), "final dynamic bit retained by strict gate");
    assert_eq!(info[m].bits.bit_count(), 2);
  }
}
