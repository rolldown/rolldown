use oxc::span::CompactStr;
use rustc_hash::FxHashSet;

/// Tracks which exports from a CJS module need to be included.
///
/// Replaces the previous binary bailout model (`FxHashSet<ModuleIdx>`) with
/// fine-grained per-export tracking. A CJS module can either need all exports
/// included (opaque/dynamic usage) or only a specific set of named exports.
#[derive(Debug)]
pub enum CjsExportInclusion {
  /// Only specific named exports are needed from this CJS module.
  Specific(FxHashSet<CompactStr>),
  /// Opaque or dynamic usage — all exports must be included.
  All,
}

impl CjsExportInclusion {
  #[inline]
  pub fn is_all(&self) -> bool {
    matches!(self, Self::All)
  }
}
