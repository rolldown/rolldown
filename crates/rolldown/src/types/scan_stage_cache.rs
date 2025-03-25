use arcstr::ArcStr;
use oxc_index::IndexVec;
use rolldown_common::GetLocalDbMut;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashMap;

use crate::{
  module_loader::module_loader::VisitState,
  stages::scan_stage::{NormalizedScanStageOutput, ScanStageOutput},
};

#[derive(Default)]
pub struct ScanStageCache {
  cache: Option<NormalizedScanStageOutput>,
  module_id_to_idx: FxHashMap<ArcStr, VisitState>,
}

impl ScanStageCache {
  #[inline]
  pub fn set_cache(&mut self, cache: NormalizedScanStageOutput) {
    self.cache = Some(cache);
  }

  pub fn set_module_id_to_idx(&mut self, module_id_to_idx: FxHashMap<ArcStr, VisitState>) {
    self.module_id_to_idx = module_id_to_idx;
  }

  pub fn take_module_id_to_idx(&mut self) -> FxHashMap<ArcStr, VisitState> {
    std::mem::take(&mut self.module_id_to_idx)
  }

  pub fn merge(&mut self, mut scan_stage_output: ScanStageOutput) {
    let Some(ref mut cache) = self.cache else {
      self.cache = Some(scan_stage_output.into());
      return;
    };
    let modules = match scan_stage_output.module_table {
      rolldown_common::HybridIndexVec::IndexVec(_index_vec) => {
        unreachable!()
      }
      rolldown_common::HybridIndexVec::Map(map) => map,
    };
    // TODO: Considering newly Added module
    //
    // merge module_table, index_ast_scope, index_ecma_ast
    for (new_idx, new_module) in modules {
      // dbg!(&new_idx);
      // dbg!(&new_module.id());
      let idx = self.module_id_to_idx[new_module.id_clone()].idx();

      let old_module = std::mem::replace(&mut cache.module_table.modules[idx], new_module);
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
    let cache = self.cache.as_mut().unwrap();
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
