use crate::{ImportRecordIdx, ModuleIdx};

/// An external module that an entry chunk re-exports as a whole: an unbroken chain of
/// `export * from` leads from the entry module to it.
/// See internal-docs/external-star-exports/implementation.md.
#[derive(Debug, Clone, Copy)]
pub struct EntryLevelExternal {
  pub external_idx: ModuleIdx,
  /// The first record in the entry's `export *` walk that has import attributes. The chunk-level
  /// re-export reuses its `with` clause.
  pub attribute_record: Option<(ModuleIdx, ImportRecordIdx)>,
}
