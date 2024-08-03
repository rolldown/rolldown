use oxc::semantic::SymbolId;

use crate::ModuleIdx;

/// Crossing module ref between symbols
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolRef {
  pub owner: ModuleIdx,
  pub symbol: SymbolId,
}

impl From<(ModuleIdx, SymbolId)> for SymbolRef {
  fn from(value: (ModuleIdx, SymbolId)) -> Self {
    Self { owner: value.0, symbol: value.1 }
  }
}
