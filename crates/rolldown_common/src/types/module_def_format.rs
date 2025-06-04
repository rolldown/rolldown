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

  /// Compared to `EcmaView::exports_kind == ExportKind::Esm`, this method ensures that not only the
  /// module is esm, but also satisfies the conditions how node.js determines if a module is esm.
  /// We use this method to determine if we want to simulate node.js's ESM execution behavior.
  pub fn is_esm(&self) -> bool {
    matches!(self, Self::EsmMjs | Self::EsmPackageJson | Self::EsmMts)
  }

  /// Same as `is_esm`, but for CommonJS modules.
  pub fn is_commonjs(&self) -> bool {
    matches!(self, Self::CJS | Self::CjsPackageJson | Self::Cts)
  }
}
