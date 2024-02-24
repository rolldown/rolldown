use oxc::semantic::SymbolId;

use crate::NormalModuleId;

/// Crossing module ref between symbols
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolRef {
  pub owner: NormalModuleId,
  pub symbol: SymbolId,
}

impl From<(NormalModuleId, SymbolId)> for SymbolRef {
  fn from(value: (NormalModuleId, SymbolId)) -> Self {
    Self { owner: value.0, symbol: value.1 }
  }
}
