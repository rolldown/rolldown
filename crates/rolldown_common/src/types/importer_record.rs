use crate::{ImportKind, ModuleId, ModuleIdx};

#[derive(Debug)]
pub struct ImporterRecord {
  pub importer_path: ModuleId,
  pub importer_idx: ModuleIdx,
  pub kind: ImportKind,
}
