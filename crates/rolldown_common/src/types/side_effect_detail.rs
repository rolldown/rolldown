use bitflags::bitflags;
bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
    /// Some statement is mark as side effects free via `Pure`, but we need to know
    /// the original statement side effects when do some runtime wrapper optimization.
    /// A global variable access with `pure` annotation, it could be eliminated when unused,
    /// but If we can't remove it's wrapper safely,because runtime behavior of global variable access maybe execution
    /// order aware
    pub struct StmtSideEffect: u8 {
        const GlobalVarAccess = 1;
        const PureCjs = 1 << 1;
        const Unknown = 1 << 2;
    }
}

impl StmtSideEffect {
  #[inline]
  pub fn has_side_effect(&self) -> bool {
    self.intersects(StmtSideEffect::PureCjs | StmtSideEffect::Unknown)
  }
}

impl From<bool> for StmtSideEffect {
  fn from(value: bool) -> Self {
    if value { StmtSideEffect::Unknown } else { StmtSideEffect::empty() }
  }
}
