use oxc::{semantic::SymbolId, span::CompactStr};

use crate::{IndexModules, Module, ModuleIdx, SymbolRefDb, SymbolRefFlags};

/// `SymbolRef` is used to represent a symbol in a module when there are multiple modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
  pub fn name<'db>(&self, db: &'db SymbolRefDb) -> &'db CompactStr {
    &db.get(*self).name
  }

  pub fn flags<'db>(&self, db: &'db SymbolRefDb) -> Option<&'db SymbolRefFlags> {
    db.get_flags(*self)
  }

  // `None` means we don't know if it's declared by `const`.
  pub fn is_declared_by_const(&self, db: &SymbolRefDb) -> Option<bool> {
    let flags = self.flags(db)?;
    if flags.contains(SymbolRefFlags::IS_CONST) {
      Some(true)
    } else {
      // Not having this flag means we don't know if it's declared by `const` instead of it's not declared by `const`.
      None
    }
  }

  /// `None` means we don't know if it gets reassigned.
  pub fn is_not_reassigned(&self, db: &SymbolRefDb) -> Option<bool> {
    let flags = self.flags(db)?;
    if flags.contains(SymbolRefFlags::IS_NOT_REASSIGNED) {
      Some(true)
    } else {
      // Not having this flag means we don't know
      None
    }
  }

  #[must_use]
  pub fn canonical_ref(&self, db: &SymbolRefDb) -> SymbolRef {
    db.par_canonical_ref_for(*self)
  }

  pub fn set_canonical_ref(&self, db: &mut SymbolRefDb, canonical_ref: SymbolRef) {
    db.link(*self, canonical_ref);
  }

  pub fn is_created_by_import_stmt_that_target_external(
    &self,
    db: &SymbolRefDb,
    modules: &IndexModules,
  ) -> bool {
    let canonical_ref = db.par_canonical_ref_for(*self);

    let Module::Normal(owner) = &modules[canonical_ref.owner] else { return false };

    let Some(named_import) = owner.named_imports.get(self) else {
      return false;
    };

    let rec = &owner.import_records[named_import.record_id];

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
