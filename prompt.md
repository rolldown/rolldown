#### 2.2 Already-Loaded Dynamic-Import Atom Propagation

**Rollup** (`chunkAssignment.ts:496-538`, `getAlreadyLoadedAtomsByEntry`): When an entry E does `import('./d')`, by the time `d` loads, *every static dep of E* and *every chunk E has already dynamically imported* is in memory. Rollup represents this as a per-dynamic-entry bigint of "atoms already loaded" and uses it to:
1. Drop `dependentEntries` from atoms whose presence is redundant (e.g. an atom shared between A and dynamic-from-A doesn't need to be its own chunk — it's already loaded).
2. Compute `correlatedAtoms` — atoms guaranteed to coexist in memory with any given chunk.

The math is a fixed-point iteration: `updatedLoadedAtoms &= staticDeps[E] | alreadyLoaded[E]` for each importer E, with re-propagation when the value changes. Comments at line 116-118 note this replaced an `O(n³)` algorithm.

**Rolldown**: `rg "already.loaded|alreadyLoadedAtoms"` in crates returns nothing. The reachability is only a forward-BFS from entry → modules; there's no analysis of *what's-already-in-memory-when-this-chunk-loads*. This means rolldown can't safely merge "this dynamic chunk's atoms into a parent if redundant" the way Rollup can.

**Impact**: For a graph like `entry → dynamic1 → shared → dynamic2`, Rollup recognizes that any module shared between `dynamic1` and `dynamic2` is already loaded (via `entry` → `dynamic1` → `shared` chain) when `dynamic2` loads, so `shared` doesn't need its own common chunk. Rolldown either creates a separate common chunk or fails to merge, depending on entry topology.

## Priority 1: Dynamic Already-Loaded Analysis

### What Rolldown Lacks

Rolldown lacks Rollup's pre-grouping dynamic import optimization. Rollup determines which chunks are already guaranteed to be loaded when a dynamic entry runs, then removes that dynamic entry from those chunks' dependent-entry sets before regrouping.

This is the biggest semantic gap. Without it, Rolldown can create extra common chunks that Rollup avoids.

Example:

```text
A -> B
A => C
C -> B
```

Initial reachability says:

```text
A: A
C: C
B: A, C
```

Rollup observes that `C` is only loaded after `A`, so `B` is already loaded when `C` runs. It removes `C` from `B`'s dependent entries:

```text
A+B in A chunk
C in C chunk
```

Rolldown may compensate in simple cases by merging the `A,C` common chunk into `A`, but that is later and narrower. It does not cover Rollup's general cases, especially when the correct result is merging or regrouping common chunks after dependent-entry reduction.

### Pseudocode

```rust
struct DynamicEntryAnalysis {
  all_entries: Vec<ModuleIdx>,
  dependent_entries_by_module: IndexVec<ModuleIdx, BitSet>,
  dynamic_imports_by_entry: Vec<BitSet>,
  dynamically_dependent_entries_by_dynamic_entry: FxHashMap<EntryIndex, BitSet>,
  awaited_dynamic_imports_by_entry: Vec<BitSet>,
  dynamically_dependent_entries_by_awaited_dynamic_entry: FxHashMap<EntryIndex, BitSet>,
}

struct ChunkAtom {
  modules: Vec<ModuleIdx>,
  dependent_entries: BitSet,
}
```

```rust
fn optimize_dynamic_entry_bits(
  entries: &[ModuleIdx],
  module_table: &ModuleTable,
  metas: &LinkingMetadataVec,
) -> Vec<ChunkAtom> {
  let analysis = analyze_module_graph(entries, module_table, metas);

  let mut atoms = group_modules_by_dependent_entries(
    &analysis.dependent_entries_by_module,
    metas,
  );

  let static_atoms_by_entry = compute_static_dependency_atoms_by_entry(
    &analysis.all_entries,
    &atoms,
  );

  let already_loaded_atoms_by_entry = compute_already_loaded_atoms_by_entry(
    &static_atoms_by_entry,
    analysis.dynamically_dependent_entries_by_dynamic_entry,
    &analysis.dynamic_imports_by_entry,
  );

  let awaited_already_loaded_atoms_by_entry = compute_already_loaded_atoms_by_entry(
    &static_atoms_by_entry,
    analysis.dynamically_dependent_entries_by_awaited_dynamic_entry,
    &analysis.awaited_dynamic_imports_by_entry,
  );

  remove_unnecessary_dependent_entries(
    &mut atoms,
    &already_loaded_atoms_by_entry,
    &awaited_already_loaded_atoms_by_entry,
  );

  regroup_atoms_by_dependent_entries(atoms)
}
```

```rust
fn compute_static_dependency_atoms_by_entry(
  all_entries: &[ModuleIdx],
  atoms: &[ChunkAtom],
) -> Vec<AtomSet> {
  let mut static_atoms_by_entry = vec![AtomSet::empty(); all_entries.len()];

  for (atom_idx, atom) in atoms.iter().enumerate() {
    for entry_idx in atom.dependent_entries.ones() {
      static_atoms_by_entry[entry_idx].insert(atom_idx);
    }
  }

  static_atoms_by_entry
}
```

```rust
fn compute_already_loaded_atoms_by_entry(
  static_atoms_by_entry: &[AtomSet],
  mut dynamically_dependent_entries_by_dynamic_entry: FxHashMap<EntryIndex, BitSet>,
  dynamic_imports_by_entry: &[BitSet],
) -> Vec<AtomSet> {
  let mut already_loaded = Vec::with_capacity(static_atoms_by_entry.len());

  for entry_idx in 0..static_atoms_by_entry.len() {
    if dynamically_dependent_entries_by_dynamic_entry.contains_key(&entry_idx) {
      already_loaded.push(AtomSet::all());
    } else {
      already_loaded.push(AtomSet::empty());
    }
  }

  while let Some((dynamic_entry_idx, dependent_entries)) =
    dynamically_dependent_entries_by_dynamic_entry.pop_next()
  {
    let known_loaded = already_loaded[dynamic_entry_idx].clone();
    let mut updated_loaded = known_loaded.clone();

    for importer_entry_idx in dependent_entries.ones() {
      updated_loaded &= static_atoms_by_entry[importer_entry_idx].clone()
        | already_loaded[importer_entry_idx].clone();
    }

    if updated_loaded != known_loaded {
      already_loaded[dynamic_entry_idx] = updated_loaded;

      for next_dynamic_entry_idx in dynamic_imports_by_entry[dynamic_entry_idx].ones() {
        dynamically_dependent_entries_by_dynamic_entry
          .entry(next_dynamic_entry_idx)
          .or_default()
          .set(dynamic_entry_idx);
      }
    }
  }

  already_loaded
}
```

```rust
fn remove_unnecessary_dependent_entries(
  atoms: &mut [ChunkAtom],
  already_loaded: &[AtomSet],
  awaited_already_loaded: &[AtomSet],
) {
  for (atom_idx, atom) in atoms.iter_mut().enumerate() {
    for entry_idx in atom.dependent_entries.ones().collect::<Vec<_>>() {
      let is_already_loaded = already_loaded[entry_idx].contains(atom_idx);
      let is_awaited_already_loaded = awaited_already_loaded[entry_idx].contains(atom_idx);

      if is_already_loaded && !is_awaited_already_loaded {
        atom.dependent_entries.unset(entry_idx);
      }
    }
  }
}
```

Integration point: run this after reachability propagation and before final common chunk materialization in `split_chunks()`.
