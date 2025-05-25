#[derive(Debug)]
pub enum StmtSideEffect {
  None,
  // TODO(hyf0): This should be removed in the future.
  Unknown,
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
