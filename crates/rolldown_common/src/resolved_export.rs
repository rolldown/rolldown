use crate::SymbolRef;

#[derive(Debug)]
pub enum ResolvedExport {
  Symbol(SymbolRef),
  Runtime(SymbolRef),
}
