use oxc::semantic::SymbolId;
use rolldown_std_utils::OptionExt;

use crate::{IndexModules, Module, ModuleIdx, ResolvedImportRecord, SymbolRefDb, SymbolRefFlags};

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

  // `None` means we don't know if it's declared by `const`.
  pub fn is_declared_by_const(&self, db: &SymbolRefDb) -> Option<bool> {
    let flags = self.flags(db)?;
    // Not having this flag means we don't know if it's declared by `const` instead of it's not declared by `const`.
    flags.contains(SymbolRefFlags::IS_CONST).then_some(true)
  }

  /// `None` means we don't know if it gets reassigned.
  pub fn is_not_reassigned(&self, db: &SymbolRefDb) -> Option<bool> {
    let flags = self.flags(db)?;
    // Not having this flag means we don't know
    flags.contains(SymbolRefFlags::IS_NOT_REASSIGNED).then_some(true)
  }

  pub fn is_declared_in_root_scope(&self, db: &SymbolRefDb) -> bool {
    db.is_declared_in_root_scope(*self)
  }

  #[must_use]
  pub fn canonical_ref(&self, db: &SymbolRefDb) -> SymbolRef {
    db.canonical_ref_for(*self)
  }

  pub fn set_canonical_ref(&self, db: &mut SymbolRefDb, canonical_ref: SymbolRef) {
    db.link(*self, canonical_ref);
  }

  pub fn is_created_by_import_stmt_that_target_external(
    &self,
    db: &SymbolRefDb,
    modules: &IndexModules,
  ) -> bool {
    let canonical_ref = db.canonical_ref_for(*self);

    let Module::Normal(owner) = &modules[canonical_ref.owner] else { return false };

    let Some(named_import) = owner.named_imports.get(self) else {
      return false;
    };

    let ResolvedImportRecord::Normal(rec) = &owner.import_records[named_import.record_id] else {
      return false;
    };

    match &modules[rec.resolved_module] {
      Module::Normal(_) => {
        // This branch should be unreachable. By `par_canonical_ref_for`, we should get the canonical ref.
        // An canonical ref is either declared by the module itself or a `import { foo } from 'bar'` statement.
        false
      }
      Module::External(_) => true,
    }
  }
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
