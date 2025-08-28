use crate::ExportsKind;

/// Information about barrel file detection
#[derive(Debug, Clone, Default)]
pub struct BarrelInfo {
  /// Number of re-export statements
  pub reexport_count: usize,
  /// Module's export kind
  pub exports_kind: ExportsKind,
  /// Whether the file has any non-import/export statements
  pub has_other_statements: bool,
}

impl BarrelInfo {
  pub fn new() -> Self {
    Self::default()
  }

  /// Check if this qualifies as a barrel file
  /// A barrel file should:
  /// 1. Be an ESM module
  /// 2. Only contain module declarations (import/export statements)
  /// 3. Have at least one re-export
  pub fn is_barrel_file(&self) -> bool {
    // Must be ESM module
    if !matches!(self.exports_kind, ExportsKind::Esm) {
      return false;
    }

    // Must not have any non-module-declaration statements
    // A pure barrel file should only contain import and export statements
    if self.has_other_statements {
      return false;
    }

    // Must have at least one re-export to be considered a barrel
    // This avoids treating files with only local exports as barrels
    self.reexport_count > 0
  }
}
