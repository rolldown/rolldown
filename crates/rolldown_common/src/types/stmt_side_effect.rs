#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum StmtSideEffect {
  #[default]
  None,
  // TODO(hyf0): This should be removed in the future.
  Unknown,
  /// e.g.
  /// - Object.defineProperty(exports, "__esModule", { value: true });
  /// - exports.a = pure_expr;
  ///   It is treated has side effect in stmt level(to preserve the export declaration), but could be treated as side effect free in
  ///   when determine if module has side effects.(like, the whole cjs module is not used at all, or
  ///   cjs module only has exports, and the cjs module it self has no side effects)
  PureCjs,
}

impl StmtSideEffect {
  pub fn has_side_effect(&self) -> bool {
    !matches!(self, StmtSideEffect::None)
  }
}

// TODO(hyf0): should be removed once we give every side effect a specific reason
impl From<bool> for StmtSideEffect {
  fn from(value: bool) -> Self {
    if value { Self::Unknown } else { StmtSideEffect::None }
  }
}
