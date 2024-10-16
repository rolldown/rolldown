use arcstr::ArcStr;

use crate::ModuleIdx;

/// A token that represents a symbol name in a module.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SymbolNameRefToken {
  owner: ModuleIdx,
  value: ArcStr,
}

impl SymbolNameRefToken {
  pub fn new(owner: ModuleIdx, value: ArcStr) -> Self {
    Self { owner, value }
  }

  pub fn owner(&self) -> ModuleIdx {
    self.owner
  }

  pub fn value(&self) -> &ArcStr {
    &self.value
  }
}
