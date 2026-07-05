use bitflags::bitflags;
bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
    /// Facts observed while evaluating a statement.
    ///
    /// These flags are shared by tree shaking and execution-order analysis. Some flags represent
    /// side effects that keep an unused statement alive; others only mean the statement's value may
    /// depend on when it is evaluated.
    pub struct StmtEvalFlags: u8 {
        /// Reads from an unresolved global or a member chain rooted at one.
        const GlobalVarAccess = 1;
        /// CJS export write that must be preserved when the statement is included.
        const PureCjs = 1 << 1;
        /// The statement may have a side effect for tree-shaking purposes.
        const UnknownSideEffect = 1 << 2;
        /// A call/new expression was marked pure by an annotation or cross-module analysis.
        const PureAnnotation = 1 << 3;
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
