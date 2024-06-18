#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ExportsKind {
  Esm,
  CommonJs,
  None,
}

impl ExportsKind {
  pub fn is_esm(&self) -> bool {
    matches!(self, Self::Esm)
  }

  pub fn is_commonjs(&self) -> bool {
    matches!(self, Self::CommonJs)
  }
}

impl Default for ExportsKind {
  fn default() -> Self {
    Self::None
  }
}
