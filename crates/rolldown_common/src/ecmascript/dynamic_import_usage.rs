use oxc::{
  semantic::{ReferenceId, SymbolId},
  span::CompactStr,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ImportRecordIdx;

#[derive(Default)]
pub struct DynamicImportUsageInfo {
  /// e.g
  /// ```js
  /// import('mod').then(mod => {
  ///   mod.test // ref1
  ///   mod // ref2
  /// })
  /// ```
  /// record all these dynamic import binding reference id
  /// used for analyze how dynamic import binding is used (partially or fully used),
  pub dynamic_import_binding_reference_id: FxHashSet<ReferenceId>,
  pub dynamic_import_binding_to_import_record_id: FxHashMap<SymbolId, ImportRecordIdx>,
  pub dynamic_import_exports_usage: FxHashMap<ImportRecordIdx, DynamicImportExportsUsage>,
}

#[derive(Debug, Clone)]
pub enum DynamicImportExportsUsage {
  Complete,
  Partial(FxHashSet<CompactStr>),
  /// This is used for insert a single export to Partial
  /// so that we don't need to create `FxHashSet` for each insertion
  Single(CompactStr),
}

impl DynamicImportExportsUsage {
  pub fn merge(&mut self, other: Self) {
    match (&mut *self, other) {
      (Self::Complete, _) => {}
      (_, Self::Complete) => {
        *self = DynamicImportExportsUsage::Complete;
      }
      (Self::Partial(lhs), rhs) => match rhs {
        DynamicImportExportsUsage::Complete => unreachable!(),
        DynamicImportExportsUsage::Partial(rhs) => {
          lhs.extend(rhs);
        }
        DynamicImportExportsUsage::Single(name) => {
          lhs.insert(name);
        }
      },
      (Self::Single(name), rhs) => {
        let set = match rhs {
          DynamicImportExportsUsage::Complete => unreachable!(),
          DynamicImportExportsUsage::Partial(mut rhs) => {
            rhs.insert(name.clone());
            rhs
          }
          DynamicImportExportsUsage::Single(rhs) => {
            let mut set = FxHashSet::default();
            set.insert(rhs);
            set.insert(name.clone());
            set
          }
        };
        *self = DynamicImportExportsUsage::Partial(set);
      }
    }
  }
}
