use oxc::span::Atom;

use super::import_record::ImportRecordId;
use crate::symbol_ref::SymbolRef;

#[derive(Debug)]
pub struct NamedImport {
  pub imported: Atom,
  pub is_imported_star: bool,
  pub imported_as: SymbolRef,
  pub record_id: ImportRecordId,
}
