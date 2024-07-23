use crate::{ImportKind, ModuleId};

#[derive(Debug)]
pub struct ImporterRecord {
  pub importer_path: ModuleId,
  pub kind: ImportKind,
}
