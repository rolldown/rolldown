use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{GetLocalDbMut, ImporterRecord, ModuleIdx};
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashMap;

use crate::{
  module_loader::module_loader::VisitState,
  stages::scan_stage::{NormalizedScanStageOutput, ScanStageOutput},
};

#[derive(Default, Debug)]
pub struct ScanStageCache {
  snapshot: Option<NormalizedScanStageOutput>,
  pub module_id_to_idx: FxHashMap<ArcStr, VisitState>,
  pub importers: IndexVec<ModuleIdx, Vec<ImporterRecord>>,
}

impl ScanStageCache {
  #[inline]
  pub fn set_snapshot(&mut self, cache: NormalizedScanStageOutput) {
    self.snapshot = Some(cache);
  }

  pub fn merge(&mut self, mut scan_stage_output: ScanStageOutput) {
    let Some(ref mut cache) = self.snapshot else {
      self.snapshot = Some(scan_stage_output.into());
      return;
    };
    let modules = match scan_stage_output.module_table {
      rolldown_common::HybridIndexVec::IndexVec(_index_vec) => {
        unreachable!()
      }
      rolldown_common::HybridIndexVec::Map(map) => {
        let mut modules = map.into_iter().collect_vec();
        modules.sort_by_key(|(k, _)| *k);
        modules
      }
    };
    // merge module_table, index_ast_scope, index_ecma_ast
    for (new_idx, mut new_module) in modules {
      let idx = self.module_id_to_idx[new_module.id_clone()].idx();

      let old_module = if new_idx.index() >= cache.module_table.modules.len() {
        if let Some(module) = new_module.as_normal_mut() {
          let new_module_idx = ModuleIdx::from_usize(cache.module_table.modules.len());
          let ecma_ast_idx = module.ecma_ast_idx();
          let new_ecma_ast_idx = cache
            .index_ecma_ast
            .push(std::mem::take(&mut scan_stage_output.index_ecma_ast[ecma_ast_idx]));
          module.ecma_ast_idx = Some(new_ecma_ast_idx);

          cache.symbol_ref_db.store_local_db(
            new_module_idx,
            std::mem::take(scan_stage_output.symbol_ref_db.local_db_mut(new_idx)),
          );
        }
        cache.module_table.modules.push(new_module);
        continue;
      } else {
        std::mem::replace(&mut cache.module_table.modules[idx], new_module)
      };
      let Some(new_module) = cache.module_table.modules[idx].as_normal_mut() else {
        continue;
      };
      let old_module = old_module.as_normal().unwrap();

      let new_ecma_ast_idx = new_module.ecma_ast_idx.expect("should have ecma_ast_idx");

      let old_ecma_ast_idx = old_module.ecma_ast_idx.expect("should have ecma_ast_idx");

      new_module.ecma_ast_idx = Some(old_ecma_ast_idx);
      std::mem::swap(
        &mut cache.index_ecma_ast[old_ecma_ast_idx],
        &mut scan_stage_output.index_ecma_ast[new_ecma_ast_idx],
      );
      std::mem::swap(
        cache.symbol_ref_db.local_db_mut(idx),
        scan_stage_output.symbol_ref_db.local_db_mut(new_idx),
      );
    }
  }

  /// # Panic
  /// the function will panic if cache is unset
  pub fn create_output(&mut self) -> NormalizedScanStageOutput {
    let cache = self.snapshot.as_mut().unwrap();
    NormalizedScanStageOutput {
      module_table: cache.module_table.clone(),
      index_ecma_ast: {
        let item = cache
          .index_ecma_ast
          .raw
          .par_iter()
          .map(|(ast, module_idx)| (ast.clone_with_another_arena(), *module_idx))
          .collect::<Vec<_>>();
        IndexVec::from_vec(item)
      },

      // Since `AstScope` is immutable in following phase, move it to avoid clone
      entry_points: cache.entry_points.clone(),
      symbol_ref_db: cache.symbol_ref_db.clone(),
      runtime: cache.runtime.clone(),
      // TODO: cache warning
      warnings: vec![],
      dynamic_import_exports_usage_map: cache.dynamic_import_exports_usage_map.clone(),
    }
  }
}
