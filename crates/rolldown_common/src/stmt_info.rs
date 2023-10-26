use crate::SymbolRef;

#[derive(Default, Debug)]
pub struct StmtInfo {
  /// The index of this statement in the module body.
  ///
  /// We will create some facade statements while bundling, and the facade statements
  /// don't have a corresponding statement in the original module body, which means
  /// `stmt_idx` will be `None`.
  pub stmt_idx: Option<usize>,
  // currently, we only store top level symbols
  pub declared_symbols: Vec<SymbolRef>,
  // We will add symbols of other modules to `referenced_symbols`, so we need `SymbolRef`
  // here instead of `SymbolId`.
  /// Top level symbols referenced by this statement.
  pub referenced_symbols: Vec<SymbolRef>,
}
