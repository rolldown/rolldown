use crate::{ImportKind, ResourceId};

#[derive(Debug)]
pub struct ImporterRecord {
  pub importer_path: ResourceId,
  pub kind: ImportKind,
}
