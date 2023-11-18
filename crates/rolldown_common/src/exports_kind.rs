#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ExportsKind {
  Esm,
  CommonJs,
  None,
}

impl Default for ExportsKind {
  fn default() -> Self {
    Self::None
  }
}
