use crate::SymbolRef;

#[derive(Debug)]
pub enum ResolvedExport {
  // Resolved static symbol. eg `export const a = 1`
  Symbol(SymbolRef),
  // Resolved symbol at runtime. eg `export { a } from 'commonjs'`
  Runtime(ResolvedExportRuntime),
}

#[derive(Debug)]
pub struct ResolvedExportRuntime {
  // It is local symbol, If `export { a } from 'commonjs'` at entry, here will create a symbol and export.
  // eg `var a = cjs_ns.a; export { a }`
  pub local: Option<SymbolRef>,
  // It is importee namespace symbol. eg it is `cjs_ns` for `export { a } from 'commonjs'`
  pub symbol_ref: SymbolRef,
}

impl ResolvedExportRuntime {
  pub fn new(symbol_ref: SymbolRef, local: Option<SymbolRef>) -> Self {
    Self { local, symbol_ref }
  }
}
