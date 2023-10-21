use crate::SymbolRef;

#[derive(Debug)]
pub enum ResolvedExport {
  // Resolved static symbol. eg `export const a = 1`
  Symbol(SymbolRef),
  // Resolved symbol at runtime. eg `export { a } from 'commonjs'`
  Runtime(SymbolRef),
}
