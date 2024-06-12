use crate::{ExternalModuleId, ImportRecord, ImportRecordId};
use oxc::index::IndexVec;

#[derive(Debug)]
pub struct ExternalModule {
  pub id: ExternalModuleId,
  pub exec_order: u32,
  pub name: String,
  pub import_records: IndexVec<ImportRecordId, ImportRecord>,
}

impl ExternalModule {
  pub fn new(id: ExternalModuleId, resource_id: String) -> Self {
    Self { id, exec_order: u32::MAX, name: resource_id, import_records: IndexVec::default() }
  }
}
