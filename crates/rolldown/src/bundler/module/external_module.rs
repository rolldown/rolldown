use index_vec::IndexVec;
use rolldown_common::{ImportRecord, ImportRecordId, NormalModuleId, ResourceId};

#[derive(Debug)]
pub struct ExternalModule {
  pub id: NormalModuleId,
  pub exec_order: u32,
  pub resource_id: ResourceId,
  pub import_records: IndexVec<ImportRecordId, ImportRecord>,
}

impl ExternalModule {
  pub fn new(id: NormalModuleId, resource_id: ResourceId) -> Self {
    Self { id, exec_order: u32::MAX, resource_id, import_records: IndexVec::default() }
  }
}
