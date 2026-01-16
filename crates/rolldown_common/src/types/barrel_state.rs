use std::ops::{Deref, DerefMut};

use oxc::span::CompactStr;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ImportRecordIdx, ModuleIdx, NormalModule, ResolvedId, Specifier,
  types::import_record::ImportRecordStateInit,
};

/// What exports are needed from a barrel module
#[derive(Debug, Clone)]
pub enum ImportedExports {
  /// `import * as x` or `import 'barrel'`
  All,
  /// `import { a, b }` or `import { a }`
  Partial(FxHashSet<CompactStr>),
}

impl ImportedExports {
  pub fn is_all(&self) -> bool {
    matches!(self, Self::All)
  }

  pub fn merge(&mut self, other: &Self) {
    match (&mut *self, other) {
      (Self::All, _) => {}
      (_, Self::All) => *self = Self::All,
      (Self::Partial(lhs), Self::Partial(rhs)) => lhs.extend(rhs.clone()),
    }
  }
}

/// Information about a barrel module's re-exports
#[derive(Debug, Default)]
pub struct BarrelInfo {
  /// `export { a } from './a'` → "a" => ImportRecordIdx
  pub export_to_record: FxHashMap<CompactStr, ImportRecordIdx>,
  /// `export * from './x'`
  pub star_export_records: Vec<ImportRecordIdx>,
  /// Remaining unprocessed import records
  pub remaining_records: FxHashSet<ImportRecordIdx>,
}

impl BarrelInfo {
  /// Check if all import records have been processed
  #[inline]
  pub fn is_fully_processed(&self) -> bool {
    self.remaining_records.is_empty()
  }

  /// Mark an import record as processed
  #[inline]
  pub fn mark_record_as_processed(&mut self, record: ImportRecordIdx) {
    self.remaining_records.remove(&record);
  }

  #[inline]
  pub fn mark_records_as_processed(&mut self, records: &[PendingBarrelRecord]) {
    for r in records {
      self.remaining_records.remove(&r.rec_idx);
    }
  }

  /// Get the import records needed based on the imported exports (excluding processed)
  pub fn get_needed_records(&self, imports: &ImportedExports) -> FxHashSet<ImportRecordIdx> {
    let mut records = FxHashSet::default();

    match imports {
      ImportedExports::All => {
        return self.remaining_records.clone();
      }
      ImportedExports::Partial(names) => {
        let mut has_missing = false;
        for name in names {
          if let Some(&rec_idx) = self.export_to_record.get(name) {
            if self.remaining_records.contains(&rec_idx) {
              records.insert(rec_idx);
            }
          } else {
            has_missing = true;
          }
        }
        if has_missing {
          // Include remaining star export records
          for &rec_idx in &self.star_export_records {
            if self.remaining_records.contains(&rec_idx) {
              records.insert(rec_idx);
            }
          }
        }
      }
    }

    records
  }

  pub fn into_barrel_module_state(
    self,
    pending_records: Vec<PendingBarrelRecord>,
  ) -> BarrelModuleState {
    BarrelModuleState { info: self, pending_records }
  }
}

/// State for lazy barrel optimization
#[derive(Debug, Default)]
pub struct BarrelState {
  /// Barrel module info (ModuleIdx -> Option<BarrelModuleState>)
  pub barrel_infos: FxHashMap<ModuleIdx, Option<BarrelModuleState>>,
  /// What's needed from each barrel module during processing
  pub initial_imported_exports: FxHashMap<ModuleIdx, ImportedExports>,
}

/// Pending import record that was skipped during barrel module processing
#[derive(Debug, Clone)]
pub struct PendingBarrelRecord {
  pub resolved_id: ResolvedId,
  pub rec_idx: ImportRecordIdx,
  pub raw_rec_state: ImportRecordStateInit,
}

/// Wrapper for BarrelInfo with pending records
#[derive(Debug, Default)]
pub struct BarrelModuleState {
  pub info: BarrelInfo,
  /// Import records that were skipped and need to be processed later
  pub pending_records: Vec<PendingBarrelRecord>,
}

impl Deref for BarrelModuleState {
  type Target = BarrelInfo;
  fn deref(&self) -> &Self::Target {
    &self.info
  }
}

impl DerefMut for BarrelModuleState {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.info
  }
}

pub fn get_record_imported_exports(
  rec_idx: ImportRecordIdx,
  normal_module: &NormalModule,
) -> ImportedExports {
  let mut is_all = false;
  let mut names = FxHashSet::default();
  for named_import in normal_module.named_imports.values() {
    if named_import.record_idx == rec_idx {
      match &named_import.imported {
        Specifier::Star => {
          is_all = true;
          break;
        }
        Specifier::Literal(name) => {
          names.insert(name.clone());
        }
      }
    }
  }
  if is_all || names.is_empty() { ImportedExports::All } else { ImportedExports::Partial(names) }
}
