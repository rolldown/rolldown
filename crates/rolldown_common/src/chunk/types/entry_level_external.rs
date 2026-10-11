use crate::{ImportRecordIdx, ModuleIdx};

/// An external module that an entry chunk re-exports at entry level. An unbroken chain of
/// `export * from` leads from the entry module to the external.
/// See internal-docs/external-star-exports/implementation.md.
#[derive(Debug, Clone, Copy)]
pub struct EntryLevelExternal {
  pub external_idx: ModuleIdx,
  /// The first record with a `with` clause, in the order of the `export *` walk of the entry. The
  /// chunk-level `export *` statement uses the same `with` clause.
  pub attribute_record: Option<(ModuleIdx, ImportRecordIdx)>,
}
