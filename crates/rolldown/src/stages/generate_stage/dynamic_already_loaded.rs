use std::collections::VecDeque;

use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  ChunkIdx, ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleIdx, PreserveEntrySignatures,
  StmtInfoIdx,
};
use rolldown_utils::BitSet;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;

use super::{GenerateStage, code_splitting::IndexSplittingInfo};

struct ChunkAtom {
  modules: Vec<ModuleIdx>,
  dependent_entries: BitSet,
}

struct DynamicEntryAnalysis {
  dynamic_entry_indices: Vec<usize>,
  dynamic_entry_modules_by_entry: Vec<Option<ModuleIdx>>,
  dynamic_imports_by_entry: Vec<BitSet>,
  dynamically_dependent_entries_by_dynamic_entry: Vec<BitSet>,
}

impl GenerateStage<'_> {
  pub(super) fn optimize_dynamic_entry_bits(
    &self,
    index_splitting_info: &mut IndexSplittingInfo,
    chunk_graph: &ChunkGraph,
    entries_len: u32,
  ) {
    let DynamicEntryAnalysis {
      dynamic_entry_indices,
      dynamic_entry_modules_by_entry,
      dynamic_imports_by_entry,
      dynamically_dependent_entries_by_dynamic_entry,
    } = self.analyze_dynamic_entries(index_splitting_info, entries_len);
    if dynamic_entry_indices.is_empty() {
      return;
    }

    let mut atoms = self.group_modules_by_dependent_entries(index_splitting_info);
    if atoms.is_empty() {
      return;
    }
    let module_to_atom_idx = self.compute_module_to_atom_idx(&atoms);
    let atom_dependencies = self.compute_atom_dependencies(&atoms, &module_to_atom_idx);

    let static_dependency_atoms_by_entry =
      Self::compute_static_dependency_atoms_by_entry(entries_len as usize, &atoms);
    let already_loaded_atoms_by_entry = Self::compute_already_loaded_atoms_by_entry(
      &static_dependency_atoms_by_entry,
      dynamically_dependent_entries_by_dynamic_entry,
      &dynamic_imports_by_entry,
      &dynamic_entry_indices,
      atoms.len(),
    );

    let mut changed = false;
    for atom_idx in 0..atoms.len() {
      let original_dependent_entries = atoms[atom_idx].dependent_entries.clone();
      let dependent_entries = atoms[atom_idx].dependent_entries.index_of_one().collect::<Vec<_>>();
      let atom_bit: u32 = atom_idx.try_into().expect("Too many atoms, u32 overflowed.");
      for entry_idx in dependent_entries {
        if already_loaded_atoms_by_entry[entry_idx as usize].has_bit(atom_bit) {
          atoms[atom_idx].dependent_entries.clear_bit(entry_idx);
        }
      }
      if atoms[atom_idx].dependent_entries != original_dependent_entries
        && self.can_use_reduced_dependent_entries(
          &atoms[atom_idx],
          &original_dependent_entries,
          &atoms[atom_idx].dependent_entries,
          chunk_graph,
          &dynamic_entry_modules_by_entry,
        )
        && (!self.should_check_reduced_atom_static_cycle(
          &original_dependent_entries,
          &atoms[atom_idx].dependent_entries,
          chunk_graph,
        ) || !Self::reduced_atom_graph_has_static_cycle(&atoms, &atom_dependencies))
      {
        changed = true;
      } else {
        atoms[atom_idx].dependent_entries = original_dependent_entries;
      }
    }

    if !changed {
      return;
    }

    for atom in atoms {
      let share_count = atom.dependent_entries.bit_count();
      for module_idx in atom.modules {
        index_splitting_info[module_idx].bits = atom.dependent_entries.clone();
        index_splitting_info[module_idx].share_count = share_count;
      }
    }
  }

  fn can_use_reduced_dependent_entries(
    &self,
    atom: &ChunkAtom,
    original_dependent_entries: &BitSet,
    dependent_entries: &BitSet,
    chunk_graph: &ChunkGraph,
    dynamic_entry_modules_by_entry: &[Option<ModuleIdx>],
  ) -> bool {
    let bit_count = dependent_entries.bit_count();
    if bit_count != 1 {
      return bit_count > 1;
    }

    let Some(entry_bit) = dependent_entries.index_of_one().next() else {
      return false;
    };
    let Some(chunk) = chunk_graph.chunk_table.get(ChunkIdx::from_raw(entry_bit)) else {
      return false;
    };

    let can_merge_without_changing_entry_signature =
      self.can_merge_without_changing_entry_signature(chunk, &atom.modules);
    let is_runtime_only_atom = self.is_runtime_only_atom(atom);
    let removed_entries_are_dynamic_entry_modules = Self::removed_entries_are_dynamic_entry_modules(
      atom,
      original_dependent_entries,
      dependent_entries,
      dynamic_entry_modules_by_entry,
    );

    if chunk.is_async_entry() {
      return can_merge_without_changing_entry_signature
        || is_runtime_only_atom
        || removed_entries_are_dynamic_entry_modules;
    }

    !matches!(chunk.preserve_entry_signature, Some(PreserveEntrySignatures::Strict))
      || can_merge_without_changing_entry_signature
      || is_runtime_only_atom
      || removed_entries_are_dynamic_entry_modules
  }

  fn is_runtime_only_atom(&self, atom: &ChunkAtom) -> bool {
    atom.modules.len() == 1 && atom.modules[0] == self.link_output.runtime.id()
  }

  fn removed_entries_are_dynamic_entry_modules(
    atom: &ChunkAtom,
    original_dependent_entries: &BitSet,
    dependent_entries: &BitSet,
    dynamic_entry_modules_by_entry: &[Option<ModuleIdx>],
  ) -> bool {
    let mut has_removed_dynamic_entry = false;
    for removed_entry_idx in
      original_dependent_entries.index_of_one().filter(|idx| !dependent_entries.has_bit(*idx))
    {
      let Some(dynamic_entry_module_idx) =
        dynamic_entry_modules_by_entry.get(removed_entry_idx as usize).copied().flatten()
      else {
        return false;
      };
      has_removed_dynamic_entry = true;
      if !atom.modules.contains(&dynamic_entry_module_idx) {
        return false;
      }
    }
    has_removed_dynamic_entry
  }

  fn should_check_reduced_atom_static_cycle(
    &self,
    original_dependent_entries: &BitSet,
    dependent_entries: &BitSet,
    chunk_graph: &ChunkGraph,
  ) -> bool {
    self.options.manual_code_splitting.is_some()
      || original_dependent_entries.index_of_one().chain(dependent_entries.index_of_one()).any(
        |entry_bit| {
          chunk_graph.chunk_table.get(ChunkIdx::from_raw(entry_bit)).is_some_and(|chunk| {
            matches!(chunk.preserve_entry_signature, Some(PreserveEntrySignatures::Strict))
          })
        },
      )
  }

  fn compute_module_to_atom_idx(&self, atoms: &[ChunkAtom]) -> IndexVec<ModuleIdx, Option<usize>> {
    let mut module_to_atom_idx = index_vec![None; self.link_output.module_table.modules.len()];
    for (atom_idx, atom) in atoms.iter().enumerate() {
      for &module_idx in &atom.modules {
        module_to_atom_idx[module_idx] = Some(atom_idx);
      }
    }
    module_to_atom_idx
  }

  fn compute_atom_dependencies(
    &self,
    atoms: &[ChunkAtom],
    module_to_atom_idx: &IndexVec<ModuleIdx, Option<usize>>,
  ) -> Vec<Vec<usize>> {
    atoms
      .iter()
      .enumerate()
      .map(|(atom_idx, atom)| {
        let mut dependencies = FxHashSet::default();
        for &module_idx in &atom.modules {
          for &dep_module_idx in &self.link_output.metas[module_idx].dependencies {
            let Some(dep_atom_idx) = module_to_atom_idx[dep_module_idx] else {
              continue;
            };
            if dep_atom_idx != atom_idx {
              dependencies.insert(dep_atom_idx);
            }
          }
        }
        dependencies.into_iter().collect()
      })
      .collect()
  }

  fn reduced_atom_graph_has_static_cycle(
    atoms: &[ChunkAtom],
    atom_dependencies: &[Vec<usize>],
  ) -> bool {
    let mut chunk_idx_by_bits = FxHashMap::default();
    let mut atom_to_chunk = Vec::with_capacity(atoms.len());
    for atom in atoms {
      let next_chunk_idx = chunk_idx_by_bits.len();
      let chunk_idx = match chunk_idx_by_bits.entry(atom.dependent_entries.clone()) {
        std::collections::hash_map::Entry::Occupied(occupied) => *occupied.get(),
        std::collections::hash_map::Entry::Vacant(vacant) => {
          vacant.insert(next_chunk_idx);
          next_chunk_idx
        }
      };
      atom_to_chunk.push(chunk_idx);
    }

    let mut chunk_dependencies = vec![FxHashSet::default(); chunk_idx_by_bits.len()];
    for (atom_idx, dependencies) in atom_dependencies.iter().enumerate() {
      let from_chunk_idx = atom_to_chunk[atom_idx];
      for &dep_atom_idx in dependencies {
        let to_chunk_idx = atom_to_chunk[dep_atom_idx];
        if from_chunk_idx != to_chunk_idx {
          chunk_dependencies[from_chunk_idx].insert(to_chunk_idx);
        }
      }
    }

    Self::chunk_dependency_graph_has_cycle(&chunk_dependencies)
  }

  fn chunk_dependency_graph_has_cycle(chunk_dependencies: &[FxHashSet<usize>]) -> bool {
    let mut state = vec![0_u8; chunk_dependencies.len()];
    for start_chunk_idx in 0..chunk_dependencies.len() {
      if state[start_chunk_idx] != 0 {
        continue;
      }

      let mut stack = vec![(start_chunk_idx, false)];
      while let Some((chunk_idx, exiting)) = stack.pop() {
        if exiting {
          state[chunk_idx] = 2;
          continue;
        }

        match state[chunk_idx] {
          1 => return true,
          2 => continue,
          _ => {}
        }

        state[chunk_idx] = 1;
        stack.push((chunk_idx, true));
        for &dep_chunk_idx in &chunk_dependencies[chunk_idx] {
          match state[dep_chunk_idx] {
            1 => return true,
            0 => stack.push((dep_chunk_idx, false)),
            _ => {}
          }
        }
      }
    }

    false
  }

  fn analyze_dynamic_entries(
    &self,
    index_splitting_info: &IndexSplittingInfo,
    entries_len: u32,
  ) -> DynamicEntryAnalysis {
    let entries_count = entries_len as usize;
    let mut dynamic_entry_indices = vec![];
    let mut dynamic_entry_modules_by_entry = vec![None; entries_count];
    let mut dynamic_imports_by_entry = vec![BitSet::new(entries_len); entries_count];
    let mut dynamically_dependent_entries_by_dynamic_entry =
      vec![BitSet::new(entries_len); entries_count];

    for (dynamic_entry_idx, (dynamic_entry_module_idx, entry_point)) in self
      .link_output
      .entries
      .iter()
      .flat_map(|(&idx, entries)| entries.iter().map(move |entry| (idx, entry)))
      .enumerate()
    {
      if !entry_point.kind.is_dynamic_import() {
        continue;
      }

      dynamic_entry_indices.push(dynamic_entry_idx);
      dynamic_entry_modules_by_entry[dynamic_entry_idx] = Some(dynamic_entry_module_idx);
      let dynamic_entry_bit: u32 =
        dynamic_entry_idx.try_into().expect("Too many entries, u32 overflowed.");

      for (importer_idx, stmt_info_idx, _address, import_record_idx) in
        &entry_point.related_stmt_infos
      {
        if !self.is_included_dynamic_import_record(
          *importer_idx,
          *stmt_info_idx,
          *import_record_idx,
          dynamic_entry_module_idx,
        ) {
          continue;
        }

        for importer_entry_idx in index_splitting_info[*importer_idx].bits.index_of_one() {
          dynamically_dependent_entries_by_dynamic_entry[dynamic_entry_idx]
            .set_bit(importer_entry_idx);
          dynamic_imports_by_entry[importer_entry_idx as usize].set_bit(dynamic_entry_bit);
        }
      }
    }

    DynamicEntryAnalysis {
      dynamic_entry_indices,
      dynamic_entry_modules_by_entry,
      dynamic_imports_by_entry,
      dynamically_dependent_entries_by_dynamic_entry,
    }
  }

  fn is_included_dynamic_import_record(
    &self,
    importer_idx: ModuleIdx,
    stmt_info_idx: StmtInfoIdx,
    import_record_idx: ImportRecordIdx,
    dynamic_entry_module_idx: ModuleIdx,
  ) -> bool {
    if !self.link_output.metas[importer_idx].stmt_info_included.has_bit(stmt_info_idx) {
      return false;
    }

    let Some(importer) = self.link_output.module_table[importer_idx].as_normal() else {
      return false;
    };
    let Some(import_record) = importer.import_records.get(import_record_idx) else {
      return false;
    };

    import_record.kind == ImportKind::DynamicImport
      && import_record.resolved_module == Some(dynamic_entry_module_idx)
      && !import_record.meta.contains(ImportRecordMeta::DeadDynamicImport)
  }

  fn group_modules_by_dependent_entries(
    &self,
    index_splitting_info: &IndexSplittingInfo,
  ) -> Vec<ChunkAtom> {
    let mut atoms = vec![];
    let mut bits_to_atom_idx = FxHashMap::default();
    for module_idx in &self.link_output.sorted_modules {
      let Some(normal_module) = self.link_output.module_table[*module_idx].as_normal() else {
        continue;
      };
      if !self.link_output.metas[normal_module.idx].is_included {
        continue;
      }

      let bits = &index_splitting_info[normal_module.idx].bits;
      if bits.is_empty() {
        continue;
      }

      let atom_idx = match bits_to_atom_idx.entry(bits.clone()) {
        std::collections::hash_map::Entry::Occupied(occupied) => *occupied.get(),
        std::collections::hash_map::Entry::Vacant(vacant) => {
          let atom_idx = atoms.len();
          atoms.push(ChunkAtom { modules: vec![], dependent_entries: bits.clone() });
          *vacant.insert(atom_idx)
        }
      };
      atoms[atom_idx].modules.push(normal_module.idx);
    }
    atoms
  }

  fn compute_static_dependency_atoms_by_entry(
    entries_count: usize,
    atoms: &[ChunkAtom],
  ) -> Vec<BitSet> {
    let atom_count: u32 = atoms.len().try_into().expect("Too many atoms, u32 overflowed.");
    let mut static_dependency_atoms_by_entry = vec![BitSet::new(atom_count); entries_count];
    for (atom_idx, atom) in atoms.iter().enumerate() {
      let atom_bit: u32 = atom_idx.try_into().expect("Too many atoms, u32 overflowed.");
      for entry_idx in atom.dependent_entries.index_of_one() {
        static_dependency_atoms_by_entry[entry_idx as usize].set_bit(atom_bit);
      }
    }
    static_dependency_atoms_by_entry
  }

  fn compute_already_loaded_atoms_by_entry(
    static_dependency_atoms_by_entry: &[BitSet],
    mut dynamically_dependent_entries_by_dynamic_entry: Vec<BitSet>,
    dynamic_imports_by_entry: &[BitSet],
    dynamic_entry_indices: &[usize],
    atom_count: usize,
  ) -> Vec<BitSet> {
    let entries_count = static_dependency_atoms_by_entry.len();
    let atom_count: u32 = atom_count.try_into().expect("Too many atoms, u32 overflowed.");
    let mut is_dynamic_entry = vec![false; entries_count];
    for &dynamic_entry_idx in dynamic_entry_indices {
      is_dynamic_entry[dynamic_entry_idx] = true;
    }

    let mut already_loaded_atoms_by_entry = is_dynamic_entry
      .iter()
      .map(|is_dynamic| if *is_dynamic { BitSet::all(atom_count) } else { BitSet::new(atom_count) })
      .collect::<Vec<_>>();

    let mut queued = vec![false; entries_count];
    let mut queue = VecDeque::new();
    for &dynamic_entry_idx in dynamic_entry_indices {
      if !dynamically_dependent_entries_by_dynamic_entry[dynamic_entry_idx].is_empty() {
        queued[dynamic_entry_idx] = true;
        queue.push_back(dynamic_entry_idx);
      }
    }

    while let Some(dynamic_entry_idx) = queue.pop_front() {
      queued[dynamic_entry_idx] = false;
      let known_loaded_atoms = already_loaded_atoms_by_entry[dynamic_entry_idx].clone();
      let mut updated_loaded_atoms = known_loaded_atoms.clone();

      for importer_entry_idx in
        dynamically_dependent_entries_by_dynamic_entry[dynamic_entry_idx].index_of_one()
      {
        let importer_entry_idx = importer_entry_idx as usize;
        let mut importer_loaded_atoms =
          static_dependency_atoms_by_entry[importer_entry_idx].clone();
        importer_loaded_atoms.union(&already_loaded_atoms_by_entry[importer_entry_idx]);
        updated_loaded_atoms.intersect(&importer_loaded_atoms);
      }

      if updated_loaded_atoms == known_loaded_atoms {
        continue;
      }

      already_loaded_atoms_by_entry[dynamic_entry_idx] = updated_loaded_atoms;
      let dynamic_entry_bit: u32 =
        dynamic_entry_idx.try_into().expect("Too many entries, u32 overflowed.");
      for next_dynamic_entry_idx in dynamic_imports_by_entry[dynamic_entry_idx].index_of_one() {
        let next_dynamic_entry_idx = next_dynamic_entry_idx as usize;
        dynamically_dependent_entries_by_dynamic_entry[next_dynamic_entry_idx]
          .set_bit(dynamic_entry_bit);
        if !queued[next_dynamic_entry_idx] {
          queued[next_dynamic_entry_idx] = true;
          queue.push_back(next_dynamic_entry_idx);
        }
      }
    }

    already_loaded_atoms_by_entry
  }
}
