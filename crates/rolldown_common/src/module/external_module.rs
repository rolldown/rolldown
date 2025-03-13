use crate::side_effects::DeterminedSideEffects;
use crate::{ImportRecordIdx, ModuleIdx, ResolvedImportRecord, SymbolRef};
use arcstr::ArcStr;
use oxc_index::IndexVec;

#[derive(Debug)]
pub struct ExternalModule {
  pub idx: ModuleIdx,
  pub exec_order: u32,
  /// Usages:
  /// - Used for iife format to inject symbol and deconflict.
  /// - Used for for rewrite `import { foo } from 'external';console.log(foo)` to `var external = require('external'); console.log(external.foo)` in cjs format.
  pub namespace_ref: SymbolRef,
  pub name: ArcStr,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
  pub side_effects: DeterminedSideEffects,
}

impl ExternalModule {
  pub fn new(
    idx: ModuleIdx,
    name: ArcStr,
    side_effects: DeterminedSideEffects,
    namespace_ref: SymbolRef,
  ) -> Self {
    Self {
      idx,
      exec_order: u32::MAX,
      namespace_ref,
      name,
      import_records: IndexVec::default(),
      side_effects,
    }
  }
}
