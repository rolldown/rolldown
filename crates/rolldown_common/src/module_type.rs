#[derive(Debug, Default, Clone, Copy)]
pub enum ModuleType {
  #[default]
  Unknown,
  // ".cjs"
  CJS,
  // "type: commonjs" in package.json
  CjsPackageJson,
  // ".mjs"
  EsmMjs,
  // "type: module" in package.json
  EsmPackageJson,
}

impl ModuleType {
  pub fn is_esm(&self) -> bool {
    matches!(self, Self::EsmMjs | Self::EsmPackageJson)
  }

  pub fn is_commonjs(&self) -> bool {
    matches!(self, Self::CJS | Self::CjsPackageJson)
  }
}
