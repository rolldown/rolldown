use oxc::semantic::SymbolId;
use rustc_hash::FxHashSet;

use crate::{IndexModules, Module, ModuleIdx, Specifier, SymbolRefDb, SymbolRefFlags};

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

  pub fn is_created_by_import_from_external(&self, modules: &IndexModules) -> bool {
    self.inner_is_created_by_import_from_external(modules, &mut FxHashSet::default())
  }
  fn inner_is_created_by_import_from_external(
    self,
    modules: &IndexModules,
    visited: &mut FxHashSet<SymbolRef>,
  ) -> bool {
    let is_not_inserted_before = visited.insert(self);
    if !is_not_inserted_before {
      // We are in a cycle
      return false;
    }

    let Module::Normal(owner) = &modules[self.owner] else { return false };

    let Some(named_import) = owner.named_imports.get(&self) else {
      return false;
    };

    let rec = &owner.import_records[named_import.record_id];

    match &modules[rec.resolved_module] {
      Module::Normal(normal) => {
        let Specifier::Literal(imported) = &named_import.imported else {
          return false;
        };
        let Some(named_export) = normal.named_exports.get(imported) else {
          return false;
        };
        named_export.referenced.is_created_by_import_from_external(modules)
      }
      Module::External(_) => true,
    }
  }
}
