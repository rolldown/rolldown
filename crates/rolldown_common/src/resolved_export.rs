use crate::{ModuleId, SymbolRef};

#[derive(Debug, Clone, Copy)]
pub struct ResolvedExport {
  pub symbol_ref: SymbolRef,
  pub export_from: Option<ModuleId>,
}
