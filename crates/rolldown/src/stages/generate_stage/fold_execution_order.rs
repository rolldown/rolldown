use oxc_index::IndexVec;
use rolldown_common::{
  ImportKind, ModuleIdx, ModuleTable, NormalModule, StmtInfo, StmtInfoMeta, SymbolRefDb, WrapKind,
  side_effects::DeterminedSideEffects,
};
use rolldown_utils::BitSet;
use rustc_hash::{FxHashMap, FxHashSet};

use super::GenerateStage;

/// The order in which each entry runs the modules it loads statically. `exec_order` comes from one
/// walk over all entries, so it gives a module the position that the first entry to reach it
/// gives it; another entry can run two modules the other way round. This walk is the one of
/// `sort_modules`, started from one entry. Entries are walked when a check first needs them.
///
/// A dynamic entry runs after the modules that every importer has loaded already
/// (`already_loaded_atoms_by_entry`), so its walk gives them no position: they ran before any
/// module of the entry, and the entries that load them check their order.
pub(super) struct EntryOrders<'a> {
  module_table: &'a ModuleTable,
  entry_module_by_bit: &'a [ModuleIdx],
  order_sensitive: &'a IndexVec<ModuleIdx, bool>,
  module_to_atom_idx: &'a IndexVec<ModuleIdx, Option<usize>>,
  already_loaded_atoms_by_entry: &'a [BitSet],
  by_entry_bit: FxHashMap<usize, EntryOrder>,
}

pub(super) struct EntryOrder {
  /// The position of each module that the entry loads statically.
  pub(super) position: FxHashMap<ModuleIdx, u32>,
  /// The modules whose position matters (see `GenerateStage::order_sensitive_modules`), with
  /// their positions, sorted by position.
  pub(super) order_sensitive: Vec<(u32, ModuleIdx)>,
  /// Whether the entry runs two modules of one atom that matter against their `exec_order`. An
  /// atom stays in one chunk, which runs them in `exec_order`, so no move can keep the order of
  /// such an entry.
  pub(super) runs_an_atom_out_of_order: bool,
}

impl<'a> EntryOrders<'a> {
  pub(super) fn new(
    module_table: &'a ModuleTable,
    entry_module_by_bit: &'a [ModuleIdx],
    order_sensitive: &'a IndexVec<ModuleIdx, bool>,
    module_to_atom_idx: &'a IndexVec<ModuleIdx, Option<usize>>,
    already_loaded_atoms_by_entry: &'a [BitSet],
  ) -> Self {
    Self {
      module_table,
      entry_module_by_bit,
      order_sensitive,
      module_to_atom_idx,
      already_loaded_atoms_by_entry,
      by_entry_bit: FxHashMap::default(),
    }
  }

  /// The orders of the entries of `entry_bits`, in that order.
  pub(super) fn walk(&mut self, entry_bits: &[usize]) -> Vec<&EntryOrder> {
    let Self {
      module_table,
      entry_module_by_bit,
      order_sensitive,
      module_to_atom_idx,
      already_loaded_atoms_by_entry,
      by_entry_bit,
    } = self;
    for &entry_bit in entry_bits {
      by_entry_bit.entry(entry_bit).or_insert_with(|| {
        walk_entry(
          module_table,
          entry_module_by_bit[entry_bit],
          order_sensitive,
          module_to_atom_idx,
          &already_loaded_atoms_by_entry[entry_bit],
        )
      });
    }
    entry_bits.iter().map(|entry_bit| &by_entry_bit[entry_bit]).collect()
  }
}

fn walk_entry(
  module_table: &ModuleTable,
  entry_module: ModuleIdx,
  order_sensitive: &IndexVec<ModuleIdx, bool>,
  module_to_atom_idx: &IndexVec<ModuleIdx, Option<usize>>,
  already_loaded_atoms: &BitSet,
) -> EntryOrder {
  enum Step {
    Enter(ModuleIdx),
    Exit(ModuleIdx),
  }
  let mut position = FxHashMap::default();
  let mut entered =
    already_loaded_modules(module_table, entry_module, module_to_atom_idx, already_loaded_atoms);
  let mut stack = vec![Step::Enter(entry_module)];
  let mut next_position = 0;
  while let Some(step) = stack.pop() {
    match step {
      Step::Enter(module_idx) => {
        if entered.insert(module_idx) {
          stack.push(Step::Exit(module_idx));
          stack.extend(
            module_table[module_idx]
              .import_records()
              .iter()
              .filter(|record| record.kind.is_static())
              .filter_map(|record| record.resolved_module)
              .rev()
              .map(Step::Enter),
          );
        }
      }
      Step::Exit(module_idx) => {
        position.insert(module_idx, next_position);
        next_position += 1;
      }
    }
  }
  let mut order_sensitive: Vec<(u32, ModuleIdx)> = position
    .iter()
    .filter(|&(&module_idx, _)| order_sensitive[module_idx])
    .map(|(&module_idx, &position)| (position, module_idx))
    .collect();
  order_sensitive.sort_unstable();
  let mut last_exec_order_by_atom = FxHashMap::default();
  let runs_an_atom_out_of_order = order_sensitive.iter().any(|&(_, module_idx)| {
    let Some(atom_idx) = module_to_atom_idx[module_idx] else {
      return false;
    };
    let exec_order = module_table[module_idx].exec_order();
    last_exec_order_by_atom.insert(atom_idx, exec_order).is_some_and(|last| last > exec_order)
  });
  EntryOrder { position, order_sensitive, runs_an_atom_out_of_order }
}

/// The modules of `already_loaded_atoms` and the externals that they import statically. A dynamic
/// entry without importers counts every atom as loaded already, its own one too, so its walk keeps
/// them all.
fn already_loaded_modules(
  module_table: &ModuleTable,
  entry_module: ModuleIdx,
  module_to_atom_idx: &IndexVec<ModuleIdx, Option<usize>>,
  already_loaded_atoms: &BitSet,
) -> FxHashSet<ModuleIdx> {
  let is_already_loaded = |module_idx: ModuleIdx| {
    module_to_atom_idx[module_idx].is_some_and(|atom_idx| {
      already_loaded_atoms.has_bit(atom_idx.try_into().expect("Too many atoms, u32 overflowed."))
    })
  };
  let mut loaded = FxHashSet::default();
  if is_already_loaded(entry_module) {
    return loaded;
  }
  let mut stack: Vec<ModuleIdx> =
    module_to_atom_idx.indices().filter(|&module_idx| is_already_loaded(module_idx)).collect();
  while let Some(module_idx) = stack.pop() {
    if loaded.insert(module_idx) {
      stack.extend(
        module_table[module_idx]
          .import_records()
          .iter()
          .filter(|record| record.kind.is_static())
          .filter_map(|record| record.resolved_module),
      );
    }
  }
  loaded
}

/// The externals whose position matters that `modules` import statically. An external import is
/// written at the top of the importing chunk, so it runs before the chunk's body.
pub(super) fn imported_order_sensitive_externals(
  modules: &[ModuleIdx],
  module_table: &ModuleTable,
  order_sensitive: &IndexVec<ModuleIdx, bool>,
) -> Vec<ModuleIdx> {
  let mut externals: Vec<ModuleIdx> = modules
    .iter()
    .filter_map(|&module_idx| module_table[module_idx].as_normal())
    .flat_map(|module| module.import_records.iter())
    .filter(|record| record.kind == ImportKind::Import)
    .filter_map(|record| record.resolved_module)
    .filter(|&dep_idx| module_table[dep_idx].is_external() && order_sensitive[dep_idx])
    .collect();
  externals.sort_unstable();
  externals.dedup();
  externals
}

impl GenerateStage<'_> {
  /// Whether the position of each module matters. A module matters when it has a side effect of
  /// its own, or when it reads, as its module evaluates, a binding of another module that can
  /// change: another module's side effect can change it, so the read must stay on its side of that
  /// side effect. An external matters when it has side effects. A wrapped module runs when an
  /// importer calls its wrapper, not when its chunk loads, and the runtime module only sets up
  /// rolldown's helpers, so neither matters.
  pub(super) fn order_sensitive_modules(&self) -> IndexVec<ModuleIdx, bool> {
    self
      .link_output
      .module_table
      .modules
      .iter()
      .map(|module| {
        let module_idx = module.idx();
        if let Some(normal_module) = module.as_normal() {
          module_idx != self.link_output.runtime.id()
            && self.link_output.metas[module_idx].wrap_kind() == WrapKind::None
            && (self.module_has_own_side_effects(normal_module)
              || self.module_reads_changing_binding(normal_module))
        } else {
          module.side_effects().has_side_effects()
        }
      })
      .collect()
  }

  fn module_has_own_side_effects(&self, module: &NormalModule) -> bool {
    match module.side_effects {
      DeterminedSideEffects::UserDefined(user_defined) => user_defined,
      DeterminedSideEffects::NoTreeshake => true,
      DeterminedSideEffects::Analyzed(false) => false,
      // The analyzed verdict is propagated from dependencies, so read the module's own statements.
      DeterminedSideEffects::Analyzed(true) => {
        let meta = &self.link_output.metas[module.idx];
        self.link_output.stmt_infos[module.idx].iter_enumerated().any(|(stmt_idx, stmt_info)| {
          meta.stmt_info_included.has_bit(stmt_idx)
            && stmt_info.eval_flags.has_side_effect_for_tree_shaking()
            && !is_plain_esm_import(stmt_info, module)
        })
      }
    }
  }

  /// Whether an included statement other than a function declaration reads a binding of another
  /// module that is not a constant: a `let` or `var` that its module can reassign later, for
  /// example from a function another module calls.
  fn module_reads_changing_binding(&self, module: &NormalModule) -> bool {
    let meta = &self.link_output.metas[module.idx];
    let symbol_db = &self.link_output.symbol_db;
    self.link_output.stmt_infos[module.idx].iter_enumerated().any(|(stmt_idx, stmt_info)| {
      meta.stmt_info_included.has_bit(stmt_idx)
        && !declares_only_functions(stmt_info, symbol_db)
        && stmt_info.referenced_symbols.iter().any(|reference| {
          let canonical = symbol_db.canonical_ref_for(*reference.symbol_ref());
          canonical.owner != module.idx
            && self.link_output.module_table[canonical.owner].is_normal()
            && !canonical.is_declared_by_const(symbol_db)
            && !canonical.is_declared_by_function(symbol_db)
            && !canonical.is_not_reassigned(symbol_db)
        })
    })
  }
}

/// A function declaration reads its references when it is called, not when its module runs.
fn declares_only_functions(stmt_info: &StmtInfo, symbol_db: &SymbolRefDb) -> bool {
  !stmt_info.declared_symbols.is_empty()
    && stmt_info
      .declared_symbols
      .iter()
      .all(|declared| declared.inner().is_declared_by_function(symbol_db))
}

/// An `import` or `export … from` runs its target at the target's own position. A re-export that
/// merges dynamic exports keeps a `__reExport` call of its own, and a `require` runs in place.
fn is_plain_esm_import(stmt_info: &StmtInfo, module: &NormalModule) -> bool {
  !stmt_info.import_records.is_empty()
    && !stmt_info.meta.contains(StmtInfoMeta::ReExportDynamicExports)
    && stmt_info
      .import_records
      .iter()
      .all(|&record_idx| module.import_records[record_idx].kind == ImportKind::Import)
}
