#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WrapKind {
  #[default]
  None,
  Cjs,
  Esm,
}

impl WrapKind {
  #[inline]
  pub fn is_none(&self) -> bool {
    matches!(self, WrapKind::None)
  }
}
