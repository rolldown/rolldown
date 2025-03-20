use crate::{ImportKind, ModuleId, ModuleIdx};

#[derive(Debug, Clone)]
pub struct ImporterRecord {
  pub importer_path: ModuleId,
  pub importer_idx: ModuleIdx,
  pub kind: ImportKind,
}
