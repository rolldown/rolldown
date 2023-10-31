use crate::{ModuleId, SymbolRef};

#[derive(Debug, Clone)]
pub struct ResolvedExport {
  pub potentially_ambiguous_symbol_refs: Option<Vec<SymbolRef>>,
  pub symbol_ref: SymbolRef,
  pub export_from: Option<ModuleId>,
}
