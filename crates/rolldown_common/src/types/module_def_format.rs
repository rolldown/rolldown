use std::path::Path;

/// Module Definition Format.
#[derive(Debug, Default, Clone, Copy)]
pub enum ModuleDefFormat {
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

impl ModuleDefFormat {
  pub fn from_path(p: impl AsRef<Path>) -> Self {
    let p = p.as_ref();

    match p.extension().and_then(|ext| ext.to_str()) {
      Some("mjs") => Self::EsmMjs,
      Some("cjs") => Self::CJS,
      _ => Self::Unknown,
    }
  }

  pub fn is_esm(&self) -> bool {
    matches!(self, Self::EsmMjs | Self::EsmPackageJson)
  }

  pub fn is_commonjs(&self) -> bool {
    matches!(self, Self::CJS | Self::CjsPackageJson)
  }
}
