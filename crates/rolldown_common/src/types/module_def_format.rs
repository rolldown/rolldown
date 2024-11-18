use std::path::Path;

/// Module Definition Format.
#[derive(Debug, Default, Clone, Copy)]
pub enum ModuleDefFormat {
  #[default]
  Unknown,
  // ".cjs"
  CJS,
  // ".cts"
  Cts,
  // "type: commonjs" in package.json
  CjsPackageJson,
  // ".mjs"
  EsmMjs,
  // ".mts"
  EsmMts,
  // "type: module" in package.json
  EsmPackageJson,
}

impl ModuleDefFormat {
  pub fn from_path(p: impl AsRef<Path>) -> Self {
    let p = p.as_ref();

    match p.extension().and_then(|ext| ext.to_str()) {
      Some("mjs") => Self::EsmMjs,
      Some("cjs") => Self::CJS,
      Some("cts") => Self::Cts,
      Some("mts") => Self::EsmMts,
      _ => Self::Unknown,
    }
  }

  pub fn is_esm(&self) -> bool {
    matches!(self, Self::EsmMjs | Self::EsmPackageJson | Self::EsmMts)
  }

  pub fn is_commonjs(&self) -> bool {
    matches!(self, Self::CJS | Self::CjsPackageJson | Self::Cts)
  }
}
