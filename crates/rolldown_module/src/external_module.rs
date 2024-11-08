use arcstr::ArcStr;
use oxc::index::IndexVec;
use rolldown_common::side_effects::DeterminedSideEffects;
use rolldown_common::{ImportRecordIdx, ModuleIdx, ResolvedImportRecord, SymbolNameRefToken};

#[derive(Debug)]
pub struct ExternalModule {
  pub idx: ModuleIdx,
  pub exec_order: u32,
  // Used for iife format to inject symbol and deconflict.
  pub name_token_for_external_binding: SymbolNameRefToken,
  pub name: ArcStr,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
  pub side_effects: DeterminedSideEffects,
}

impl ExternalModule {
  pub fn new(
    idx: ModuleIdx,
    module_id: ArcStr,
    side_effects: DeterminedSideEffects,
    name_token_for_external_binding: SymbolNameRefToken,
  ) -> Self {
    Self {
      idx,
      exec_order: u32::MAX,
      name_token_for_external_binding,
      name: module_id,
      import_records: IndexVec::default(),
      side_effects,
    }
  }
}
