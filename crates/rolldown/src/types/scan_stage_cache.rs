use arcstr::ArcStr;
use rolldown_common::{GetLocalDbMut, ModuleIdx, SymbolRef};
use rustc_hash::FxHashMap;

use crate::{
  stages::scan_stage::{NormalizedScanStageOutput, ScanStageOutput},
  type_alias::IndexAstScope,
};

#[derive(Default)]
pub struct ScanStageCache {
  cache: Option<NormalizedScanStageOutput>,
  module_id_to_idx: FxHashMap<ArcStr, ModuleIdx>,
}

impl ScanStageCache {
  pub fn set_cache(&mut self, cache: NormalizedScanStageOutput) {
    self.cache = Some(cache);
  }

  pub fn set_module_id_to_idx(&mut self, module_id_to_idx: FxHashMap<ArcStr, ModuleIdx>) {
    self.module_id_to_idx = module_id_to_idx;
  }

  // # Panic
  // - if `cache` is unset
  pub fn merge(
    &mut self,
    mut scan_stage_output: ScanStageOutput,
    changed_module_ids_remapping: FxHashMap<ModuleIdx, ModuleIdx>,
  ) {
    let Some(ref mut cache) = self.cache else { unreachable!() };
    // TODO: Considering newly Added module
    // merge module_table, index_ast_scope, index_ecma_ast
    for new_module in scan_stage_output.module_table.modules {
      let idx = self.module_id_to_idx[&new_module.id_clone()];
      let new_idx = changed_module_ids_remapping.get(&idx).copied().unwrap_or(idx);
      let old_module = std::mem::replace(&mut cache.module_table.modules[idx], new_module);
      let Some(new_module) = cache.module_table.modules[idx].as_normal_mut() else {
        continue;
      };
      let mut old_module = old_module.try_into_normal().unwrap();
      let new_ecma_ast_idx = new_module.ecma_ast_idx.expect("should have ecma_ast_idx");
      let new_ast_scope_idx = new_module.ast_scope_idx.expect("should have ast_scope_idx");

      let old_ecma_ast_idx = old_module.ecma_ast_idx.expect("should have ecma_ast_idx");
      let old_ast_scope_idx = old_module.ast_scope_idx.expect("should have ast_scope_idx");

      std::mem::swap(
        &mut cache.index_ecma_ast[old_ecma_ast_idx],
        &mut scan_stage_output.index_ecma_ast[new_ecma_ast_idx],
      );

      std::mem::swap(
        &mut cache.index_ast_scope[old_ast_scope_idx],
        &mut scan_stage_output.index_ast_scope[new_ast_scope_idx],
      );

      std::mem::swap(
        cache.symbol_ref_db.local_db_mut(idx),
        scan_stage_output
          .symbol_ref_db
          .get_mut(new_idx)
          .as_mut()
          .expect("should have symbol_ref_db"),
      );

      new_module.idx = idx;
      new_module.ecma_ast_idx = Some(old_ecma_ast_idx);
      new_module.ast_scope_idx = Some(old_ast_scope_idx);
      new_module.importers = std::mem::take(&mut old_module.importers);
      // Fixing owner idx
      for ele in new_module.stmt_infos.infos.iter_mut() {
        for ele in ele.declared_symbols.iter_mut() {
          ele.owner = idx;
        }

        for ele in ele.referenced_symbols.iter_mut() {
          match ele {
            rolldown_common::SymbolOrMemberExprRef::Symbol(sym) => {
              sym.owner = idx;
            }
            rolldown_common::SymbolOrMemberExprRef::MemberExpr(member_expr) => {
              member_expr.object_ref.owner = idx;
            }
          }
        }
      }
      // TODO: optimize
      new_module.stmt_infos.symbol_ref_to_declared_stmt_idx = new_module
        .stmt_infos
        .symbol_ref_to_declared_stmt_idx
        .iter()
        .map(|(k, v)| {
          let key = SymbolRef { owner: idx, symbol: k.symbol };
          let value = v.clone();
          (key, value)
        })
        .collect();
      cache.index_ecma_ast[old_ecma_ast_idx].1 = idx;
      cache.symbol_ref_db.local_db_mut(idx).owner_idx = idx;
    }
  }

  /// # Panic
  /// the function will panic if cache is unset
  pub fn create_output(&mut self) -> NormalizedScanStageOutput {
    let cache = self.cache.as_mut().unwrap();
    NormalizedScanStageOutput {
      module_table: cache.module_table.clone(),
      index_ecma_ast: cache
        .index_ecma_ast
        .iter()
        .map(|(ast, module_idx)| (ast.clone_with_another_arena(), *module_idx))
        .collect(),
      // Since `AstScope` is immutable in following phase, move it to avoid clone
      index_ast_scope: std::mem::take(&mut cache.index_ast_scope),
      entry_points: cache.entry_points.clone(),
      symbol_ref_db: cache.symbol_ref_db.clone(),
      runtime: cache.runtime.clone(),
      // TODO: cache warning
      warnings: vec![],
      dynamic_import_exports_usage_map: cache.dynamic_import_exports_usage_map.clone(),
    }
  }

  /// # Panic
  /// The function will panic if the cache is unset
  pub fn set_ast_scopes(&mut self, index_ast_scope: IndexAstScope) {
    self.cache.as_mut().unwrap().index_ast_scope = index_ast_scope;
  }

  pub fn module_id_to_idx(&self) -> &FxHashMap<ArcStr, ModuleIdx> {
    &self.module_id_to_idx
  }
}
