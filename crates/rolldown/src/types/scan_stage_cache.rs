use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{
  BarrelState, GetLocalDbMut, ImporterRecord, Module, ModuleId, ModuleIdx, StableModuleId,
};
use rolldown_error::BuildResult;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath;

use crate::{
  SharedOptions, SharedResolver,
  module_loader::{deferred_scan_data::defer_sync_scan_data, module_loader::VisitState},
  stages::scan_stage::{NormalizedScanStageOutput, ScanStageOutput},
};

#[derive(Default, Debug)]
pub struct ScanStageCache {
  snapshot: Option<NormalizedScanStageOutput>,
  pub barrel_state: BarrelState,
  pub module_id_to_idx: FxHashMap<ModuleId, VisitState>,
  pub importers: IndexVec<ModuleIdx, Vec<ImporterRecord>>,
  pub user_defined_entry: FxHashSet<ModuleId>,
  // Usage: Map file path emitted by watcher to corresponding module index
  pub module_idx_by_abs_path: FxHashMap<ArcStr, ModuleIdx>,
  // Usage: Map module stable id injected to client code to corresponding module index
  pub module_idx_by_stable_id: FxHashMap<StableModuleId, ModuleIdx>,
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

  /// Useful when workarounding rustc borrow rules
  pub fn take_snapshot(&mut self) -> Option<NormalizedScanStageOutput> {
    self.snapshot.take()
  }

  pub async fn update_defer_sync_data(
    &mut self,
    options: &SharedOptions,
    resolver: &SharedResolver,
  ) -> BuildResult<()> {
    let snapshot = self.take_snapshot();
    if let Some(mut snapshot) = snapshot {
      defer_sync_scan_data(options, resolver, &self.module_id_to_idx, &mut snapshot).await?;
      self.set_snapshot(snapshot);
    }
    Ok(())
  }

  /// # Panic
  /// - if the snapshot is unset
  pub fn get_snapshot(&self) -> &NormalizedScanStageOutput {
    self.snapshot.as_ref().unwrap()
  }

  pub fn merge(&mut self, mut scan_stage_output: ScanStageOutput) -> BuildResult<()> {
    let Some(ref mut cache) = self.snapshot else {
      self.snapshot = Some(
        scan_stage_output.try_into().map_err(|e: &'static str| vec![anyhow::anyhow!(e).into()])?,
      );
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
    // merge module_table, index_ast_scope, index_ecma_ast
    for (new_idx, new_module) in modules {
      let idx = self.module_id_to_idx[new_module.id()].idx();

      // Update `module_idx_by_abs_path`
      if let rolldown_common::Module::Normal(normal_module) = &new_module {
        self
          .module_idx_by_abs_path
          .insert(normal_module.id.as_arc_str().to_slash().unwrap().into(), normal_module.idx);
      }
      // Update `module_idx_by_stable_id`
      self.module_idx_by_stable_id.insert(new_module.stable_id().clone(), new_module.idx());

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
          .map(|(module_idx, _, _, _)| *module_idx)
          .collect::<FxHashSet<_>>();
        _ = old_entry_point
          .related_stmt_infos
          .extract_if(.., |(module_idx, _stmt_info_idx, _address, _)| {
            removed_module_idxs.contains(module_idx)
          });
        old_entry_point.related_stmt_infos.extend(entry_point.related_stmt_infos);
      } else {
        cache.entry_points.push(entry_point);
      }
    }

    // Update barrel module resolved import records
    let resolved_barrel_modules = std::mem::take(&mut self.barrel_state.resolved_barrel_modules);
    for (barrel_module_idx, resolved_imports) in resolved_barrel_modules {
      let barrel_module = &mut cache.module_table[barrel_module_idx];
      if let Module::Normal(normal_module) = barrel_module {
        resolved_imports.into_iter().for_each(|(rec_idx, new_idx)| {
          normal_module.import_records[rec_idx].resolved_module = Some(new_idx);
        });
      }
    }

    Ok(())
  }

  fn build_module_index_maps(&mut self, build_snapshot: &NormalizedScanStageOutput) {
    self.module_idx_by_abs_path.clear();
    self.module_idx_by_stable_id.clear();

    for module in &build_snapshot.module_table.modules {
      if let rolldown_common::Module::Normal(normal_module) = module {
        let filename = normal_module.id.as_arc_str().to_slash().unwrap().into();
        let module_idx = normal_module.idx;
        self.module_idx_by_abs_path.insert(filename, module_idx);
      }
      self.module_idx_by_stable_id.insert(module.stable_id().clone(), module.idx());
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
