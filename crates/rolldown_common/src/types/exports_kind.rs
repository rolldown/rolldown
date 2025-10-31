#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum ExportsKind {
  Esm,
  CommonJs,
  #[default]
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
