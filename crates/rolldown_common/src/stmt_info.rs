use oxc::semantic::SymbolId;

use crate::SymbolRef;

index_vec::define_index_type! {
  pub struct StmtInfoId = u32;
}

#[derive(Default, Debug)]
pub struct StmtInfo {
  pub stmt_idx: usize,
  // currently, we only store top level symbols
  pub declared_symbols: Vec<SymbolId>,
  // We will add symbols of other modules to `referenced_symbols`, so we need `SymbolRef`
  // here instead of `SymbolId`.
  /// Top level symbols referenced by this statement.
  pub referenced_symbols: Vec<SymbolRef>,
}

// Because we want declare symbols at linker, it shouldn't mutate the original `StmtInfo`.
#[derive(Default, Debug)]
pub struct VirtualStmtInfo {
  pub declared_symbols: Vec<SymbolId>,
  pub referenced_symbols: Vec<SymbolRef>,
}
