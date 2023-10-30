#[derive(Debug, Default)]
pub enum WrapKind {
  #[default]
  None,
  CJS,
  ESM,
}
