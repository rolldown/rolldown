use bitflags::bitflags;
bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
    /// Facts used by tree shaking while evaluating a statement.
    pub struct StmtEvalFlags: u8 {
        /// CJS export write that must be preserved when the statement is included.
        const PureCjs = 1;
        /// The statement may have a side effect for tree-shaking purposes.
        const UnknownSideEffect = 1 << 1;
    }
}

impl StmtEvalFlags {
  #[inline]
  pub fn has_side_effect_for_tree_shaking(&self) -> bool {
    self.intersects(StmtEvalFlags::PureCjs | StmtEvalFlags::UnknownSideEffect)
  }
}

impl From<bool> for StmtEvalFlags {
  fn from(value: bool) -> Self {
    if value { StmtEvalFlags::UnknownSideEffect } else { StmtEvalFlags::empty() }
  }
}
