#[derive(Debug, Default)]
pub enum WrapKind {
  #[default]
  None,
  Cjs,
  Esm,
}
