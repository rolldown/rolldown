use index_vec::IndexVec;
use rolldown_common::{ExternalModuleId, ImportRecord, ImportRecordId, ResourceId};

#[derive(Debug)]
pub struct ExternalModule {
  pub id: ExternalModuleId,
  pub exec_order: u32,
  pub resource_id: ResourceId,
  pub import_records: IndexVec<ImportRecordId, ImportRecord>,
}

impl ExternalModule {
  pub fn new(id: ExternalModuleId, resource_id: ResourceId) -> Self {
    Self { id, exec_order: u32::MAX, resource_id, import_records: IndexVec::default() }
  }
}
