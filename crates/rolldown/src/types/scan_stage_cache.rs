use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{GetLocalDbMut, ImporterRecord, ModuleIdx};
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath;

use crate::{
  module_loader::module_loader::VisitState,
  stages::scan_stage::{NormalizedScanStageOutput, ScanStageOutput},
};

#[derive(Default, Debug)]
pub struct ScanStageCache {
  snapshot: Option<NormalizedScanStageOutput>,
  pub module_id_to_idx: FxHashMap<ArcStr, VisitState>,
  pub importers: IndexVec<ModuleIdx, Vec<ImporterRecord>>,
  pub user_defined_entry: FxHashSet<ArcStr>,
  // Usage: Map file path emitted by watcher to corresponding module index
  pub module_idx_by_abs_path: FxHashMap<ArcStr, ModuleIdx>,
  // Usage: Map module stable id injected to client code to corresponding module index
  pub module_idx_by_stable_id: FxHashMap<String, ModuleIdx>,
}

impl ScanStageCache {
  #[inline]
  pub fn set_snapshot(&mut self, cache: NormalizedScanStageOutput) {
    self.build_module_index_maps(&cache);
    self.snapshot = Some(cache);
  }

  /// # Panic
  /// - if the snapshot is unset
  pub fn get_snapshot_mut(&mut self) -> &mut NormalizedScanStageOutput {
    self.snapshot.as_mut().unwrap()
  }

  /// # Panic
  /// - if the snapshot is unset
  pub fn get_snapshot(&self) -> &NormalizedScanStageOutput {
    self.snapshot.as_ref().unwrap()
  }

  pub fn merge(&mut self, mut scan_stage_output: ScanStageOutput) -> Result<(), &'static str> {
    let Some(ref mut cache) = self.snapshot else {
      self.snapshot = Some(scan_stage_output.try_into()?);
      return Ok(());
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
    for (idx, symbols) in scan_stage_output.safely_merge_cjs_ns_map {
      match cache.safely_merge_cjs_ns_map.entry(idx) {
        std::collections::hash_map::Entry::Occupied(mut occupied_entry) => {
          let owners = symbols.iter().map(|item| item.owner).collect::<FxHashSet<ModuleIdx>>();
          let cache_symbols = occupied_entry.get_mut();
          cache_symbols.retain(|symbol| !owners.contains(&symbol.owner));
          cache_symbols.extend(symbols);
        }
        std::collections::hash_map::Entry::Vacant(vacant_entry) => {
          vacant_entry.insert(symbols);
        }
      }
    }
    // merge module_table, index_ast_scope, index_ecma_ast
    for (new_idx, new_module) in modules {
      let idx = self.module_id_to_idx[new_module.id_clone()].idx();

      // Update `module_idx_by_abs_path`
      if let rolldown_common::Module::Normal(normal_module) = &new_module {
        self
          .module_idx_by_abs_path
          .insert(normal_module.id.resource_id().to_slash().unwrap().into(), normal_module.idx);
      }
      // Update `module_idx_by_stable_id`
      self.module_idx_by_stable_id.insert(new_module.stable_id().to_string(), new_module.idx());

      if new_idx.index() >= cache.module_table.modules.len() {
        let new_module_idx = ModuleIdx::from_usize(cache.module_table.modules.len());

        cache.symbol_ref_db.store_local_db(
          new_module_idx,
          std::mem::take(scan_stage_output.symbol_ref_db.local_db_mut(new_idx)),
        );
        cache.module_table.modules.push(new_module);
        cache.index_ecma_ast.push(scan_stage_output.index_ecma_ast.get_mut(new_idx).take());
        continue;
      }
      cache.module_table[idx] = new_module;
      cache.index_ecma_ast[idx] = scan_stage_output.index_ecma_ast.get_mut(new_idx).take();
      std::mem::swap(
        cache.symbol_ref_db.local_db_mut(idx),
        scan_stage_output.symbol_ref_db.local_db_mut(new_idx),
      );
    }

    // merge entries
    for entry_point in scan_stage_output.entry_points {
      if let Some(old_entry_point) = cache
        .entry_points
        .iter_mut()
        .find(|old_entry| old_entry.kind == entry_point.kind && old_entry.idx == entry_point.idx)
      {
        let removed_module_idxs = entry_point
          .related_stmt_infos
          .iter()
          .map(|(module_idx, _, _)| *module_idx)
          .collect::<FxHashSet<_>>();
        _ = old_entry_point.related_stmt_infos.extract_if(.., |(module_idx, _stmt_info_idx, _)| {
          removed_module_idxs.contains(module_idx)
        });
        old_entry_point.related_stmt_infos.extend(entry_point.related_stmt_infos);
      } else {
        cache.entry_points.push(entry_point);
      }
    }
    Ok(())
  }

  fn build_module_index_maps(&mut self, build_snapshot: &NormalizedScanStageOutput) {
    self.module_idx_by_abs_path.clear();
    self.module_idx_by_stable_id.clear();

    for module in &build_snapshot.module_table.modules {
      if let rolldown_common::Module::Normal(normal_module) = module {
        let filename = normal_module.id.resource_id().to_slash().unwrap().into();
        let module_idx = normal_module.idx;
        self.module_idx_by_abs_path.insert(filename, module_idx);
      }
      self.module_idx_by_stable_id.insert(module.stable_id().to_string(), module.idx());
    }
  }

  /// # Panic
  /// the function will panic if cache is unset
  pub fn create_output(&mut self) -> NormalizedScanStageOutput {
    let cache = self.snapshot.as_mut().unwrap();
    // Only clone the mutated part of symbol_ref_db
    let symbol_ref_db_partial = cache.symbol_ref_db.clone_without_scoping();
    let symbol_ref_db = std::mem::take(&mut cache.symbol_ref_db);
    cache.symbol_ref_db = symbol_ref_db_partial;

    NormalizedScanStageOutput {
      module_table: cache.module_table.clone(),
      index_ecma_ast: {
        let item = cache
          .index_ecma_ast
          .raw
          .par_iter()
          .map(|ast| ast.as_ref().map(rolldown_ecmascript::EcmaAst::clone_with_another_arena))
          .collect::<Vec<_>>();
        IndexVec::from_vec(item)
      },
      safely_merge_cjs_ns_map: cache.safely_merge_cjs_ns_map.clone(),

      // Since `AstScope` is immutable in following phase, move it to avoid clone
      entry_points: cache.entry_points.clone(),
      symbol_ref_db,
      runtime: cache.runtime.clone(),
      // TODO: cache warning
      warnings: vec![],
      dynamic_import_exports_usage_map: cache.dynamic_import_exports_usage_map.clone(),
      overrode_preserve_entry_signature_map: cache.overrode_preserve_entry_signature_map.clone(),
      entry_point_to_reference_ids: cache.entry_point_to_reference_ids.clone(),
      flat_options: cache.flat_options,
    }
  }
}
