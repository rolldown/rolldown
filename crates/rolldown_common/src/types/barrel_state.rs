use std::{collections::hash_map::Entry, ops::Deref};

use oxc::span::CompactStr;
use oxc_index::IndexVec;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  EcmaView, ImportRecordIdx, ImportRecordMeta, ModuleIdx, NormalModule, RawImportRecord,
  ResolvedId, Specifier, types::import_record::ImportRecordStateInit,
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
  #[inline]
  pub fn is_all(&self) -> bool {
    matches!(self, Self::All)
  }

  #[inline]
  pub fn is_partial(&self) -> bool {
    matches!(self, Self::Partial(_))
  }

  pub fn merge(&mut self, other: &Self) {
    match (&mut *self, other) {
      (Self::All, _) => {}
      (_, Self::All) => *self = Self::All,
      (Self::Partial(lhs), Self::Partial(rhs)) => lhs.extend(rhs.clone()),
    }
  }

  pub fn is_subset_of(&self, other: &Self) -> bool {
    match (self, other) {
      (_, ImportedExports::All) => true,
      (ImportedExports::All, ImportedExports::Partial(_)) => false,
      (ImportedExports::Partial(a), ImportedExports::Partial(b)) => a.iter().all(|x| b.contains(x)),
    }
  }

  pub fn subtract_and_merge_into(self, other: &mut Self) -> Option<Self> {
    match (self, other) {
      (_, Self::All) => None,
      (Self::All, other @ Self::Partial(_)) => {
        *other = Self::All;
        Some(Self::All)
      }
      (Self::Partial(mut a), Self::Partial(b)) => {
        a.retain(|x| !b.contains(x));
        if a.is_empty() {
          return None;
        }
        b.extend(a.iter().cloned());
        Some(Self::Partial(a))
      }
    }
  }
}

/// Information about a barrel module's re-exports
#[derive(Debug, Default)]
pub struct BarrelInfo {
  /// `export { a as b } from './x'` → "b" => (ImportRecordIdx, Literal("a"))
  /// `export * as ns from './x'` → "ns" => (ImportRecordIdx, Star)
  pub export_to_record: FxHashMap<CompactStr, (ImportRecordIdx, Specifier)>,
  /// `export * from './x'`
  pub star_export_records: Vec<ImportRecordIdx>,
}

impl BarrelInfo {
  /// Get the import records needed based on the imported exports.
  /// If `tracked_records` is None, all records are considered needed.
  /// If `tracked_records` is Some, only records in the map are considered.
  pub fn get_needed_records<T>(
    &self,
    imports: &ImportedExports,
    tracked_records: Option<&FxHashMap<ImportRecordIdx, T>>,
  ) -> FxHashMap<ImportRecordIdx, ImportedExports> {
    let mut records = FxHashMap::default();
    match imports {
      ImportedExports::All => {
        if let Some(tr) = tracked_records {
          records.reserve(tr.len());
          for &rec_idx in tr.keys() {
            records.insert(rec_idx, ImportedExports::All);
          }
        } else {
          records.reserve(self.export_to_record.len() + self.star_export_records.len());
          for &(rec_idx, _) in self.export_to_record.values() {
            records.insert(rec_idx, ImportedExports::All);
          }
          for &rec_idx in &self.star_export_records {
            records.insert(rec_idx, ImportedExports::All);
          }
        }
      }
      ImportedExports::Partial(names) => {
        records.reserve(names.len());
        let mut missing_names = FxHashSet::default();
        for name in names {
          if let Some(&(rec_idx, ref imported)) = self.export_to_record.get(name) {
            if tracked_records.is_none_or(|tr| tr.contains_key(&rec_idx)) {
              match imported {
                // `export * as ns from './x'` - request All from source
                Specifier::Star => {
                  records.insert(rec_idx, ImportedExports::All);
                }
                // `export { c as d } from './x'` - use imported_name "c", not export name "d"
                Specifier::Literal(imported_name) => match records.entry(rec_idx) {
                  Entry::Occupied(mut occ) => {
                    if let ImportedExports::Partial(set) = occ.get_mut() {
                      set.insert(imported_name.clone());
                    }
                  }
                  Entry::Vacant(vac) => {
                    vac.insert(ImportedExports::Partial(FxHashSet::from_iter([
                      imported_name.clone()
                    ])));
                  }
                },
              }
            }
          } else {
            missing_names.insert(name.clone());
          }
        }
        if !missing_names.is_empty() {
          let missing = ImportedExports::Partial(missing_names);
          for &rec_idx in &self.star_export_records {
            if tracked_records.is_none_or(|tr| tr.contains_key(&rec_idx)) {
              match records.entry(rec_idx) {
                Entry::Occupied(mut occ) => occ.get_mut().merge(&missing),
                Entry::Vacant(vac) => {
                  vac.insert(missing.clone());
                }
              }
            }
          }
        }
      }
    }
    records
  }

  pub fn into_barrel_module_state(
    self,
    tracked_records: FxHashMap<ImportRecordIdx, (ImportRecordStateInit, ResolvedId)>,
  ) -> BarrelModuleState {
    BarrelModuleState { info: self, tracked_records }
  }
}

/// State for lazy barrel optimization
#[derive(Debug, Default)]
pub struct BarrelState {
  /// Barrel module info (ModuleIdx -> Option<BarrelModuleState>)
  pub barrel_infos: FxHashMap<ModuleIdx, Option<BarrelModuleState>>,
  /// Requested exports for barrel modules
  pub requested_exports: FxHashMap<ModuleIdx, ImportedExports>,
}

/// Wrapper for BarrelInfo with pending records
#[derive(Debug, Default)]
pub struct BarrelModuleState {
  pub info: BarrelInfo,
  pub tracked_records: FxHashMap<ImportRecordIdx, (ImportRecordStateInit, ResolvedId)>,
}

impl Deref for BarrelModuleState {
  type Target = BarrelInfo;
  fn deref(&self) -> &Self::Target {
    &self.info
  }
}

#[expect(clippy::implicit_hasher)]
pub fn take_imported_specifiers(
  rec_idx: ImportRecordIdx,
  normal_module: &NormalModule,
  needed_records: Option<&FxHashMap<ImportRecordIdx, ImportedExports>>,
  all_imported_specifiers: &mut Option<FxHashMap<ImportRecordIdx, ImportedExports>>,
) -> ImportedExports {
  if let Some(imported_exports) = needed_records.and_then(|m| m.get(&rec_idx)) {
    return imported_exports.clone();
  }
  let cache = all_imported_specifiers.get_or_insert_with(|| {
    let mut result = FxHashMap::default();
    for named_import in normal_module.named_imports.values() {
      match &named_import.imported {
        Specifier::Star => {
          result.insert(named_import.record_idx, ImportedExports::All);
        }
        Specifier::Literal(name) => match result.entry(named_import.record_idx) {
          Entry::Occupied(mut occ) => {
            if let ImportedExports::Partial(set) = occ.get_mut() {
              set.insert(name.clone());
            }
          }
          Entry::Vacant(vac) => {
            vac.insert(ImportedExports::Partial(FxHashSet::from_iter([name.clone()])));
          }
        },
      }
    }
    result
  });
  cache.remove(&rec_idx).unwrap_or(ImportedExports::Partial(FxHashSet::default()))
}

/// Build BarrelInfo from EcmaView for lazy barrel optimization.
pub fn try_extract_barrel_info(
  ecma_view: &EcmaView,
  raw_import_records: &IndexVec<ImportRecordIdx, RawImportRecord>,
) -> Option<BarrelInfo> {
  // Check if module has side effects - barrel modules must be side-effect free
  if ecma_view.side_effects.has_side_effects() || raw_import_records.is_empty() {
    return None;
  }

  let mut star_export_records = Vec::new();
  let mut export_to_record = FxHashMap::default();

  // Find re-exports from named_imports
  // `export * as ns from './x'`: export_name="ns", imported=Star
  // `export { c as d } from './x'`: export_name="d", imported=Literal("c")
  for (export_name, local_export) in &ecma_view.named_exports {
    if let Some(named_import) = ecma_view.named_imports.get(&local_export.referenced) {
      // We only care about re-exports here
      if raw_import_records[named_import.record_idx].meta.contains(ImportRecordMeta::IsReExport) {
        export_to_record
          .insert(export_name.clone(), (named_import.record_idx, named_import.imported.clone()));
      }
    }
  }

  // Find star exports from import records
  for (rec_idx, record) in raw_import_records.iter_enumerated() {
    if record.meta.contains(ImportRecordMeta::IsExportStar) {
      star_export_records.push(rec_idx);
    }
  }

  // Only return Some if there are any re-exports
  if export_to_record.is_empty() && star_export_records.is_empty() {
    None
  } else {
    Some(BarrelInfo { export_to_record, star_export_records })
  }
}
