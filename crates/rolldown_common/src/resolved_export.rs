use oxc::semantic::ReferenceId;

use crate::SymbolRef;

#[derive(Debug)]
pub struct ResolvedExport {
  pub local_symbol: SymbolRef,
  pub local_ref: ReferenceId,
}
