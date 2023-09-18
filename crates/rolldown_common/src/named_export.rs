use oxc::span::Atom;

use super::import_record::ImportRecordId;
use crate::symbol_ref::SymbolRef;

#[derive(Debug)]
pub struct LocalExport {
  pub referenced: SymbolRef,
}

impl From<LocalExport> for LocalOrReExport {
  fn from(value: LocalExport) -> Self {
    Self::Local(value)
  }
}

#[derive(Debug)]
pub struct ReExport {
  pub imported: Atom,
  pub is_imported_star: bool,
  pub record_id: ImportRecordId,
}

impl From<ReExport> for LocalOrReExport {
  fn from(value: ReExport) -> Self {
    Self::Re(value)
  }
}

#[derive(Debug)]
pub enum LocalOrReExport {
  Local(LocalExport),
  Re(ReExport),
}
