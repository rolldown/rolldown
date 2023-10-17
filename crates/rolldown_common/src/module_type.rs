#[derive(Debug, Default, Clone, Copy)]
pub enum ModuleType {
  #[default]
  Unknown,
  // "c.js"
  CJS,
  // ".mjs"
  EsmMjs,
  // "type: module" in package.json
  EsmPackageJson,
}

impl ModuleType {
  pub fn is_esm(&self) -> bool {
    matches!(self, ModuleType::EsmMjs | ModuleType::EsmPackageJson)
  }
}
