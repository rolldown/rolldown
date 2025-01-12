#[derive(Debug, Default, Clone, Copy)]
pub enum WrapKind {
  #[default]
  None,
  Cjs,
  Esm,
}
