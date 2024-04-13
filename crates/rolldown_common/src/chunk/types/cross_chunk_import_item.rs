use crate::{Specifier, SymbolRef};

#[derive(Debug, Clone)]
pub struct CrossChunkImportItem {
  pub export_alias: Option<Specifier>,
  pub import_ref: SymbolRef,
}
