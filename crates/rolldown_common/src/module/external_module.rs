use crate::side_effects::DeterminedSideEffects;
use crate::{ImportRecord, ImportRecordIdx, ModuleIdx};
use oxc::index::IndexVec;

#[derive(Debug)]
pub struct ExternalModule {
  pub idx: ModuleIdx,
  pub exec_order: u32,
  pub name: String,
  pub import_records: IndexVec<ImportRecordIdx, ImportRecord>,
  pub side_effects: DeterminedSideEffects,
}

impl ExternalModule {
  pub fn new(idx: ModuleIdx, module_id: String, side_effects: DeterminedSideEffects) -> Self {
    Self {
      idx,
      exec_order: u32::MAX,
      name: module_id,
      import_records: IndexVec::default(),
      side_effects,
    }
  }
}
