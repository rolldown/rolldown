use crate::{ImportRecordId, ModuleId};

index_vec::define_index_type! {
    pub struct PartId = u32;
}

#[derive(Debug, Default, Clone)]
pub struct Part {
  pub module_id: ModuleId,
  pub start: usize,
  pub end: usize,
  pub import_record_id: Option<ImportRecordId>,
}

impl Part {
  pub fn new(
    module_id: ModuleId,
    start: usize,
    end: usize,
    import_record_id: Option<ImportRecordId>,
  ) -> Self {
    Self {
      module_id,
      start,
      end,
      import_record_id,
    }
  }
}
