use std::collections::hash_map::Entry;

use oxc::span::CompactStr;
use oxc_index::IndexVec;
use rolldown_utils::rustc_hash::FxHashMapExt as _;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  EcmaView, ImportRecordIdx, ImportRecordMeta, ModuleIdx, NormalModule, RawImportRecord,
  ResolvedId, Specifier, side_effects::DeterminedSideEffects,
  types::import_record::ImportRecordStateInit,
};

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
  pub fn get_needed_records(
    &mut self,
    imports: &ImportedExports,
    all_imported_specifiers: &mut FxHashMap<ImportRecordIdx, ImportedExports>,
  ) -> Option<FxHashMap<ImportRecordIdx, ImportedExports>> {
    match imports {
      ImportedExports::All => None,
      ImportedExports::Partial(names) => {
        let mut needs_records = FxHashMap::with_capacity(names.len());
        let mut missing_names = FxHashSet::default();
        for name in names {
          if let Some((rec_idx, ref imported)) = self.export_to_record.remove(name) {
            match imported {
              // `export * as ns from './x'` - request All from source
              Specifier::Star => {
                needs_records.insert(rec_idx, ImportedExports::All);
                all_imported_specifiers.remove(&rec_idx);
              }
              // `export { c as d } from './x'` - use imported_name "c", not export name "d"
              Specifier::Literal(imported_name) => {
                match all_imported_specifiers.entry(rec_idx) {
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
                match needs_records.entry(rec_idx) {
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
          for rec_idx in &self.star_export_records {
            match needs_records.entry(*rec_idx) {
              Entry::Occupied(mut occ) => occ.get_mut().merge(&missing),
              Entry::Vacant(vac) => {
                vac.insert(missing.clone());
              }
            }
          }
        }
        Some(needs_records)
      }
    }
  }

  pub fn into_barrel_module_state(
    self,
    tracked_records: FxHashMap<ImportRecordIdx, (ImportRecordStateInit, ResolvedId)>,
    remaining_imported_specifiers: FxHashMap<ImportRecordIdx, ImportedExports>,
  ) -> BarrelModuleState {
    assert!(
      tracked_records.len() == remaining_imported_specifiers.len(),
      "Tracked records and remaining specifiers must have the same length"
    );
    BarrelModuleState { info: self, tracked_records, remaining_imported_specifiers }
  }
}

/// State for lazy barrel optimization
#[derive(Debug, Default)]
pub struct BarrelState {
  pub barrel_infos: FxHashMap<ModuleIdx, Option<BarrelModuleState>>,
  pub requested_exports: FxHashMap<ModuleIdx, ImportedExports>,
}

impl BarrelState {
  pub fn initialize_barrel_tracking(
    &mut self,
    module: &NormalModule,
    barrel_info: &mut Option<BarrelInfo>,
  ) -> (
    FxHashMap<ImportRecordIdx, ImportedExports>,
    Option<FxHashMap<ImportRecordIdx, ImportedExports>>,
  ) {
    // Track which imported specifiers each import record needs.
    // If `requested_exports` exists later, only the corresponding specifiers will be recorded for the resolved module.
    // If `requested_exports` is None or `barrel_info` is None, all specifiers of the import record will be recorded.
    // Note: `import '..'` and `export * from '..'` are not included here and need separate handling.
    // Currently, `export { } from '..'` is treated as `import '..'`.
    let mut all_imported_specifiers = FxHashMap::default();
    for named_import in module.named_imports.values() {
      match &named_import.imported {
        Specifier::Star => {
          all_imported_specifiers.insert(named_import.record_idx, ImportedExports::All);
        }
        Specifier::Literal(name) => match all_imported_specifiers.entry(named_import.record_idx) {
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

    // Determine initial needed records from requested exports for this barrel module.
    //
    // Returns `None` (load all import records) when:
    // - `requested_exports` is `None`: module is an entry point or loaded via `this.load`/`this.emit`/`fetch_mode`
    // - `requested_exports` is `All`: module is fully imported (e.g., DynamicImport, Require, NewUrl, HotAccept, or `this.load`/`this.emit`)
    // - `barrel_info` is `None`: module is not a side-effect-free barrel module with re-exports
    //
    // Returns `Some` (load only partial re-export import records) when all conditions are met for lazy barrel optimization.
    let initial_needed_records =
      self.requested_exports.get(&module.idx).and_then(|imported_exports| {
        barrel_info
          .as_mut()
          .and_then(|info| info.get_needed_records(imported_exports, &mut all_imported_specifiers))
      });

    (all_imported_specifiers, initial_needed_records)
  }
}

/// Wrapper for BarrelInfo with pending records
#[derive(Debug, Default)]
pub struct BarrelModuleState {
  pub info: BarrelInfo,
  pub tracked_records: FxHashMap<ImportRecordIdx, (ImportRecordStateInit, ResolvedId)>,
  pub remaining_imported_specifiers: FxHashMap<ImportRecordIdx, ImportedExports>,
}

/// Build BarrelInfo from EcmaView for lazy barrel optimization.
pub fn try_extract_barrel_info(
  ecma_view: &EcmaView,
  raw_import_records: &IndexVec<ImportRecordIdx, RawImportRecord>,
) -> Option<BarrelInfo> {
  // Check if module has side effects - barrel modules must be user-defined side-effect-free
  if !matches!(ecma_view.side_effects, DeterminedSideEffects::UserDefined(false))
    || raw_import_records.is_empty()
  {
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
