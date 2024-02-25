use rolldown_common::SymbolRef;

#[derive(Debug, PartialEq, Eq)]
pub enum MatchImportKind {
  NotFound,
  // The import symbol will generate property access to namespace symbol
  Namespace(SymbolRef),
  // External,
  PotentiallyAmbiguous(SymbolRef, Vec<SymbolRef>),
  Found(SymbolRef),
}
