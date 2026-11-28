use crate::SymbolRef;

#[derive(Debug, Clone)]
pub struct CrossChunkImportItem {
  pub import_ref: SymbolRef,
  pub import_chunk_namespace: bool,
}
