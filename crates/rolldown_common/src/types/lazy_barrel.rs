use std::collections::hash_map::Entry;

use oxc::span::CompactStr;
use oxc_index::IndexVec;
use rolldown_utils::rustc_hash::{FxHashMapExt as _, FxHashSetExt as _};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  EcmaView, ImportKind, ImportRecordIdx, ImportRecordMeta, ImportRecordStateInit, ModuleIdx,
  NormalModule, RawImportRecord, ResolvedId, Specifier, side_effects::DeterminedSideEffects,
};

/// State for lazy barrel optimization
#[derive(Debug, Default)]
pub struct BarrelState {
  pub barrel_infos: FxHashMap<ModuleIdx, Option<LazyBarrelInfo>>,
  pub requested_exports: FxHashMap<ModuleIdx, ImportedExports>,
  // Used for incremental updates to track which barrel modules have had their imports resolved
  pub resolved_barrel_modules: FxHashMap<ModuleIdx, Vec<(ImportRecordIdx, ModuleIdx)>>,
}

impl BarrelState {
  /// Initialize barrel tracking for a module during the module loading phase.
  ///
  /// This method prepares the data structures needed for lazy barrel optimization by:
  /// 1. Building a map of all import records to their required imported exports
  /// 2. Determining which records need to be loaded initially vs deferred
  ///
  /// # Returns
  /// A tuple of two maps:
  /// - `imported_exports_per_record`: Remaining import records after `initial_needed_records`
  ///   consumes (fully or partially) from the original map. Used for tracking records
  ///   that may be loaded later on-demand.
  /// - `initial_needed_records`: Records that must be loaded immediately based on
  ///   the currently requested exports from this module.
  pub fn initialize_barrel_tracking(
    &self,
    module: &NormalModule,
    raw_import_records: &IndexVec<ImportRecordIdx, RawImportRecord>,
    barrel_info: &mut Option<BarrelInfo>,
  ) -> (FxHashMap<ImportRecordIdx, ImportedExports>, FxHashMap<ImportRecordIdx, ImportedExports>)
  {
    let mut imported_exports_per_record = FxHashMap::default();
    for named_import in module.named_imports.values() {
      match &named_import.imported {
        Specifier::Star => {
          imported_exports_per_record.insert(named_import.record_idx, ImportedExports::All);
        }
        Specifier::Literal(name) => {
          match imported_exports_per_record.entry(named_import.record_idx) {
            Entry::Occupied(mut occ) => {
              if let ImportedExports::Partial(set) = occ.get_mut() {
                set.insert(name.clone());
              }
            }
            Entry::Vacant(vac) => {
              vac.insert(ImportedExports::Partial(FxHashSet::from_iter([name.clone()])));
            }
          }
        }
      }
    }

    for (rec_idx, rec) in raw_import_records.iter_enumerated() {
      if rec.kind != ImportKind::Import {
        continue;
      }
      if rec.meta.contains(ImportRecordMeta::IsExportStar) {
        imported_exports_per_record.insert(rec_idx, ImportedExports::All);
      } else {
        imported_exports_per_record
          .entry(rec_idx)
          .or_insert_with(|| ImportedExports::Partial(FxHashSet::default()));
      }
    }

    let initial_needed_records = match barrel_info {
      Some(barrel_info) => barrel_info.take_needed_records(
        self.requested_exports.get(&module.idx).unwrap_or(&ImportedExports::All),
        &mut imported_exports_per_record,
      ),
      None => std::mem::take(&mut imported_exports_per_record),
    };

    (imported_exports_per_record, initial_needed_records)
  }
}

/// What exports are needed from a module
#[derive(Debug, Clone)]
pub enum ImportedExports {
  All,
  Partial(FxHashSet<CompactStr>),
}

impl ImportedExports {
  pub fn merge(&mut self, other: &Self) {
    match (&mut *self, other) {
      (Self::All, _) => {}
      (_, Self::All) => *self = Self::All,
      (Self::Partial(lhs), Self::Partial(rhs)) => lhs.extend(rhs.clone()),
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

#[derive(Debug)]
pub struct ExportSource {
  imported: Specifier,
  record_idx: ImportRecordIdx,
}

#[derive(Debug, Default)]
pub struct BarrelInfo {
  /// `export const a = 1`
  pub local: Vec<CompactStr>,
  /// `export * from './x'`
  pub star: Vec<ImportRecordIdx>,
  /// `export { a } from './x'` or `export * as ns from './x'`
  pub named: FxHashMap<CompactStr, ExportSource>,
}

impl BarrelInfo {
  /// Determine which import records need to be loaded based on requested exports.
  ///
  /// This method consumes entries from `imported_exports_per_record` and returns
  /// the records that must be loaded to satisfy `imports`.
  ///
  /// # Behavior
  /// - If `imports` is `All`: returns all records (takes entire `imported_exports_per_record`)
  /// - If `imports` is `Partial` with names:
  ///   - Named exports are resolved to their source records
  ///   - Missing names are searched in star re-exports
  ///   - If any local (own) export is used, all non-re-export import records must be loaded
  ///     (since the barrel module itself needs to execute)
  ///
  /// # Side effects
  /// - Consumes matched entries from `imported_exports_per_record`
  /// - Removes matched entries from `self.named`
  /// - Clears `self.local` if a local export is used
  pub fn take_needed_records(
    &mut self,
    imports: &ImportedExports,
    imported_exports_per_record: &mut FxHashMap<ImportRecordIdx, ImportedExports>,
  ) -> FxHashMap<ImportRecordIdx, ImportedExports> {
    match imports {
      ImportedExports::All => std::mem::take(imported_exports_per_record),
      ImportedExports::Partial(names) if names.is_empty() => FxHashMap::default(),
      ImportedExports::Partial(names) => {
        let has_local_export = self.local.iter().any(|name| names.contains(name));
        let mut needs_records = FxHashMap::with_capacity(names.len());
        let mut missing_names = FxHashSet::default();
        for name in names {
          if let Some(export_source) = self.named.remove(name) {
            match export_source.imported {
              // `export * as ns from './x'` - request All from source
              Specifier::Star => {
                needs_records.insert(export_source.record_idx, ImportedExports::All);
                imported_exports_per_record.remove(&export_source.record_idx);
              }
              // `export { c as d } from './x'` - use imported_name "c", not export name "d"
              Specifier::Literal(ref imported_name) => {
                match imported_exports_per_record.entry(export_source.record_idx) {
                  Entry::Occupied(mut occ) => {
                    let ImportedExports::Partial(set) = occ.get_mut() else {
                      unreachable!(
                        "If specifier is Literal, the imported specifiers must be Partial"
                      );
                    };
                    if set.len() <= 1 {
                      occ.remove();
                    } else {
                      set.remove(imported_name);
                    }
                  }
                  Entry::Vacant(_) => {}
                }
                match needs_records.entry(export_source.record_idx) {
                  Entry::Occupied(mut occ) => {
                    let ImportedExports::Partial(set) = occ.get_mut() else {
                      unreachable!(
                        "If specifier is Literal, the imported specifiers must be Partial"
                      );
                    };
                    set.insert(imported_name.clone());
                  }
                  Entry::Vacant(vac) => {
                    vac.insert(ImportedExports::Partial(FxHashSet::from_iter([
                      imported_name.clone()
                    ])));
                  }
                }
              }
            }
          } else {
            missing_names.insert(name.clone());
          }
        }
        if !missing_names.is_empty() {
          let missing = ImportedExports::Partial(missing_names);
          for rec_idx in &self.star {
            match needs_records.entry(*rec_idx) {
              Entry::Occupied(mut occ) => occ.get_mut().merge(&missing),
              Entry::Vacant(vac) => {
                vac.insert(missing.clone());
              }
            }
          }
        }
        if has_local_export {
          let mut reexports = FxHashSet::with_capacity(self.named.len() + self.star.len());
          reexports.extend(self.named.values().map(|v| v.record_idx));
          reexports.extend(self.star.iter().copied());
          imported_exports_per_record.retain(|rec_idx, rec| match needs_records.entry(*rec_idx) {
            Entry::Occupied(_) => true,
            Entry::Vacant(vac) => {
              if reexports.contains(rec_idx) {
                vac.insert(ImportedExports::Partial(FxHashSet::default()));
                true
              } else {
                vac.insert(rec.clone());
                false
              }
            }
          });
          self.local.clear();
        }
        needs_records
      }
    }
  }

  pub fn into_barrel_module_state(
    self,
    tracked_records: FxHashMap<ImportRecordIdx, (ImportRecordStateInit, ResolvedId)>,
    remaining_imported_specifiers: FxHashMap<ImportRecordIdx, ImportedExports>,
  ) -> LazyBarrelInfo {
    assert!(
      tracked_records.len() == remaining_imported_specifiers.len(),
      "Tracked records and remaining specifiers must have the same length"
    );
    LazyBarrelInfo { info: self, tracked_records, remaining_imported_specifiers }
  }
}

#[derive(Debug)]
pub struct LazyBarrelInfo {
  pub info: BarrelInfo,
  pub remaining_imported_specifiers: FxHashMap<ImportRecordIdx, ImportedExports>,
  pub tracked_records: FxHashMap<ImportRecordIdx, (ImportRecordStateInit, ResolvedId)>,
}

/// Try to extract BarrelInfo from EcmaView for lazy barrel optimization
pub fn try_extract_lazy_barrel_info(
  ecma_view: &EcmaView,
  raw_import_records: &IndexVec<ImportRecordIdx, RawImportRecord>,
) -> Option<BarrelInfo> {
  // Barrel modules must be user-defined side-effect-free and have import records
  if !matches!(ecma_view.side_effects, DeterminedSideEffects::UserDefined(false))
    || raw_import_records.is_empty()
  {
    return None;
  }

  let mut barrel_info = BarrelInfo::default();

  // Find star exports from import records
  for (rec_idx, record) in raw_import_records.iter_enumerated() {
    if record.meta.contains(ImportRecordMeta::IsExportStar) {
      barrel_info.star.push(rec_idx);
    }
  }

  // Categorize exports into named re-exports and local (own) exports
  // - Re-exports: `export * as ns from './x'` or `export { c as d } from './x'`
  // - Local exports: `export const a = 1` or `export function foo() {}`
  for (export_name, local_export) in &ecma_view.named_exports {
    if let Some(named_import) = ecma_view.named_imports.get(&local_export.referenced) {
      // Re-export: the export references an import
      barrel_info.named.insert(
        export_name.clone(),
        ExportSource {
          record_idx: named_import.record_idx,
          imported: named_import.imported.clone(),
        },
      );
    } else {
      // Local export: the export is defined in this module
      barrel_info.local.push(export_name.clone());
    }
  }

  if barrel_info.star.is_empty() && barrel_info.named.is_empty() && barrel_info.local.is_empty() {
    None
  } else {
    Some(barrel_info)
  }
}
