use crate::side_effects::{self, DeterminedSideEffects};
use crate::{ExternalModuleIdx, ImportRecord, ImportRecordIdx};
use oxc::index::IndexVec;

#[derive(Debug)]
pub struct ExternalModule {
  pub idx: ExternalModuleIdx,
  pub exec_order: u32,
  pub name: String,
  pub import_records: IndexVec<ImportRecordIdx, ImportRecord>,
  pub side_effects: DeterminedSideEffects,
}

impl ExternalModule {
  pub fn new(
    id: ExternalModuleIdx,
    resource_id: String,
    side_effects: DeterminedSideEffects,
  ) -> Self {
    Self {
      idx: id,
      exec_order: u32::MAX,
      name: resource_id,
      import_records: IndexVec::default(),
      side_effects,
    }
  }
}
