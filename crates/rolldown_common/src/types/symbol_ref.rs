use oxc::semantic::SymbolId;
use rolldown_std_utils::OptionExt;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  EcmaViewMeta, ImportKind, IndexModules, Module, ModuleIdx, SymbolRefDb, SymbolRefFlags,
};

use super::symbol_ref_db::{GetLocalDb, GetLocalDbMut};

/// `SymbolRef` is used to represent a symbol in a module when there are multiple modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolRef {
  pub owner: ModuleIdx,
  pub symbol: SymbolId,
}

impl From<(ModuleIdx, SymbolId)> for SymbolRef {
  fn from(value: (ModuleIdx, SymbolId)) -> Self {
    Self { owner: value.0, symbol: value.1 }
  }
}

impl SymbolRef {
  pub fn name<'db>(&self, db: &'db SymbolRefDb) -> &'db str {
    db[self.owner].unpack_ref().ast_scopes.symbol_name(self.symbol)
  }

  pub fn set_name(&self, db: &mut SymbolRefDb, name: &str) {
    db[self.owner].unpack_ref_mut().ast_scopes.set_symbol_name(self.symbol, name);
  }

  /// Not all symbols have flags info, we only care about part of them.
  /// If you want to ensure the flags info exists, use `flags_mut` instead.
  pub fn flags<'db, T: GetLocalDb>(&self, db: &'db T) -> Option<&'db SymbolRefFlags> {
    db.local_db(self.owner).flags.get(&self.symbol)
  }

  pub fn flags_mut<'db, T: GetLocalDbMut>(&self, db: &'db mut T) -> &'db mut SymbolRefFlags {
    db.local_db_mut(self.owner).flags.entry(self.symbol).or_default()
  }

  pub fn is_declared_by_const(&self, db: &SymbolRefDb) -> bool {
    db.local_db(self.owner).ast_scopes.scoping().symbol_flags(self.symbol).is_const_variable()
  }

  /// Whether the binding is guaranteed never reassigned. A missing flag means we don't know,
  /// which is treated conservatively as "possibly reassigned" (`false`).
  pub fn is_not_reassigned(&self, db: &SymbolRefDb) -> bool {
    self.flags(db).is_some_and(|flags| flags.contains(SymbolRefFlags::IsNotReassigned))
  }

  pub fn is_side_effect_free_function(&self, db: &SymbolRefDb, modules: &IndexModules) -> bool {
    let mut static_import_cycle_cache = FxHashMap::default();
    // Without a caller module, variable-initialized functions must stay conservative because
    // their initialization order cannot be checked.
    self.is_side_effect_free_function_with_cycle_cache(
      db,
      modules,
      self.owner,
      &mut static_import_cycle_cache,
    )
  }

  pub fn is_side_effect_free_function_with_cycle_cache(
    &self,
    db: &SymbolRefDb,
    modules: &IndexModules,
    callsite_module_idx: ModuleIdx,
    static_import_cycle_cache: &mut FxHashMap<ModuleIdx, bool>,
  ) -> bool {
    let Some(normal_module) = modules[self.owner].as_normal() else {
      return false;
    };
    if !normal_module.meta.contains(EcmaViewMeta::TopExportedSideEffectsFreeFunction) {
      return false;
    }
    if normal_module.meta.has_eval() {
      return false;
    }
    let Some(flag) = self.flags(db) else {
      return false;
    };
    if !flag.contains(SymbolRefFlags::IsNotReassigned)
      || !flag.contains(SymbolRefFlags::SideEffectsFreeFunction)
    {
      return false;
    }
    if flag.contains(SymbolRefFlags::VarInitializedSideEffectsFreeFunction)
      && self.owner == callsite_module_idx
    {
      return false;
    }
    if flag.intersects(
      SymbolRefFlags::VarInitializedSideEffectsFreeFunction
        | SymbolRefFlags::DelayedDefaultExportSideEffectsFreeFunction,
    ) {
      if *static_import_cycle_cache
        .entry(self.owner)
        .or_insert_with(|| module_has_static_import_cycle(self.owner, modules))
      {
        return false;
      }
      if *static_import_cycle_cache
        .entry(callsite_module_idx)
        .or_insert_with(|| module_has_static_import_cycle(callsite_module_idx, modules))
      {
        return false;
      }
    }
    true
  }

  pub fn is_declared_in_root_scope(&self, db: &SymbolRefDb) -> bool {
    db.is_declared_in_root_scope(*self)
  }

  #[must_use]
  pub fn canonical_ref(&self, db: &SymbolRefDb) -> SymbolRef {
    db.canonical_ref_for(*self)
  }

  pub fn is_created_by_import_stmt_that_target_external(
    &self,
    db: &SymbolRefDb,
    modules: &IndexModules,
  ) -> bool {
    let canonical_ref = db.canonical_ref_for(*self);
    let Module::Normal(owner) = &modules[canonical_ref.owner] else { return false };

    let Some(module_idx) = owner
      .named_imports
      .get(self)
      .and_then(|named_import| owner.import_records[named_import.record_idx].resolved_module)
    else {
      return false;
    };

    match &modules[module_idx] {
      Module::Normal(_) => {
        // This branch should be unreachable. By `par_canonical_ref_for`, we should get the canonical ref.
        // An canonical ref is either declared by the module itself or a `import { foo } from 'bar'` statement.
        false
      }
      Module::External(_) => true,
    }
  }
}

fn module_has_static_import_cycle(module_idx: ModuleIdx, modules: &IndexModules) -> bool {
  let mut seen = FxHashSet::default();
  has_static_import_path_to(module_idx, module_idx, modules, &mut seen)
}

fn has_static_import_path_to(
  target: ModuleIdx,
  current: ModuleIdx,
  modules: &IndexModules,
  seen: &mut FxHashSet<ModuleIdx>,
) -> bool {
  let Some(module) = modules[current].as_normal() else {
    return false;
  };
  module.import_records.iter().any(|record| {
    if !matches!(record.kind, ImportKind::Import | ImportKind::Require) {
      return false;
    }
    let Some(next) = record.resolved_module else {
      return false;
    };
    if next == target {
      return true;
    }
    seen.insert(next) && has_static_import_path_to(target, next, modules, seen)
  })
}

/// passing a `SymbolRef`, it will return it's string repr, the format:
/// `${stable_id} -> ${symbol_name}`
pub fn common_debug_symbol_ref(
  symbol_ref: SymbolRef,
  modules: &IndexModules,
  symbols: &SymbolRefDb,
) -> String {
  format!("{:?} -> {:?}", modules[symbol_ref.owner].stable_id(), symbol_ref.name(symbols))
}
