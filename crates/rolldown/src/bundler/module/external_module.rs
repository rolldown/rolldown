use index_vec::IndexVec;
use rolldown_common::{ImportRecord, ImportRecordId, ModuleId, ResourceId};

#[derive(Debug)]
pub struct ExternalModule {
  pub id: ModuleId,
  pub exec_order: u32,
  pub resource_id: ResourceId,
  pub import_records: IndexVec<ImportRecordId, ImportRecord>,
}

impl ExternalModule {
  pub fn new(id: ModuleId, resource_id: ResourceId) -> Self {
    Self { id, exec_order: u32::MAX, resource_id, import_records: IndexVec::default() }
  }
}
