use crate::{FilePath, ImportKind};

#[derive(Debug)]
pub struct ImporterRecord {
  pub importer_path: FilePath,
  pub kind: ImportKind,
}
