use std::sync::Arc;

use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{
  BarrelState, EcmaModuleAstUsage, GetLocalDbMut, ImporterRecord, Module, ModuleId, ModuleIdx,
  ResolvedId, StableModuleId,
};
use rolldown_error::BuildResult;
use rolldown_plugin::PluginDriver;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath;

use crate::{
  SharedOptions,
  module_loader::{deferred_scan_data::defer_sync_scan_data, module_loader::VisitState},
  stages::scan_stage::{NormalizedScanStageOutput, ScanStageOutput},
};

#[derive(Default, Debug)]
pub struct ScanStageCache {
  snapshot: Option<NormalizedScanStageOutput>,
  pub barrel_state: BarrelState,
  pub module_id_to_idx: FxHashMap<ModuleId, VisitState>,
  pub importers: IndexVec<ModuleIdx, Vec<ImporterRecord>>,
  /// Modules whose `importers` records were mutated by a partial scan.
  /// [`Self::merge`] re-derives their materialized importer sets, which the
  /// scan does only for re-scanned modules.
  pub modules_with_changed_importers: FxHashSet<ModuleIdx>,
  /// Files of an aborted (and reverted) partial scan; the next partial scan
  /// retries them, so their errors keep surfacing until the files are fixed.
  /// Only files the graph still needs are queued; see
  /// [`ModuleLoader::revert_partial_scan`](crate::module_loader::module_loader::ModuleLoader).
  pub pending_rescans: Vec<ResolvedId>,
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

  pub async fn update_defer_sync_data(&mut self, options: &SharedOptions) -> BuildResult<()> {
    if let Some(mut snapshot) = self.take_snapshot() {
      // `defer_sync_scan_data` mutates `snapshot` in place; restore it on every
      // outcome. Bailing with `?` would drop it, leaving `self.snapshot == None`
      // and panicking the next HMR cycle's `get_snapshot()`. A partially-synced
      // snapshot is recoverable; a missing one is not.
      // See internal-docs/bundler-data-lifecycle/implementation.md ("Cache integrity on a failed build").
      let result = defer_sync_scan_data(options, &self.module_id_to_idx, &mut snapshot).await;
      self.set_snapshot(snapshot);
      result?;
    }
    Ok(())
  }

  /// # Panic
  /// - if the snapshot is unset
  pub fn get_snapshot(&self) -> &NormalizedScanStageOutput {
    self.snapshot.as_ref().unwrap()
  }

  /// Non-panicking variant of [`Self::get_snapshot`].
  pub fn snapshot(&self) -> Option<&NormalizedScanStageOutput> {
    self.snapshot.as_ref()
  }

  /// Between builds the cache is either empty (no snapshot: only a full scan
  /// is possible) or a valid graph any partial scan can build on. A failed
  /// partial scan keeps the latter true by reverting its mutations
  /// (`ModuleLoader::revert_partial_scan`).
  pub fn has_snapshot(&self) -> bool {
    self.snapshot.is_some()
  }

  /// Re-derives the `importers` edge list (one record per resolved import
  /// record, keyed by the imported module) from the snapshot. Restores the
  /// pre-scan list up to slot order; consumers treat slots as sets.
  ///
  /// # Panic
  /// - if the snapshot is unset
  pub fn derive_importers_from_snapshot(&self) -> IndexVec<ModuleIdx, Vec<ImporterRecord>> {
    let snapshot = self.get_snapshot();
    let mut importers = IndexVec::from_vec(
      std::iter::repeat_with(Vec::new).take(snapshot.module_table.modules.len()).collect(),
    );
    for module in &snapshot.module_table.modules {
      let Some(module) = module.as_normal() else {
        continue;
      };
      for record in &module.ecma_view.import_records {
        if let Some(dep_idx) = record.resolved_module {
          importers[dep_idx].push(ImporterRecord {
            importer_path: module.id.clone(),
            importer_idx: module.idx,
            kind: record.kind,
          });
        }
      }
    }
    importers
  }

  pub fn merge(
    &mut self,
    mut scan_stage_output: ScanStageOutput,
    plugin_driver: &PluginDriver,
  ) -> BuildResult<()> {
    fn module_has_tla(module: &Module) -> bool {
      module.as_normal().is_some_and(|normal_module| {
        normal_module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait)
      })
    }

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
        modules.sort_unstable_by_key(|(k, _)| *k);
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
          .insert(ArcStr::from(normal_module.id.as_arc_str().to_slash()), normal_module.idx);
      }
      // Update `module_idx_by_stable_id`
      self.module_idx_by_stable_id.insert(new_module.stable_id().clone(), new_module.idx());

      let incoming_tla_span = scan_stage_output.tla_keyword_span_map.get(&new_idx).copied();

      if new_idx.index() >= cache.module_table.modules.len() {
        let new_module_idx = ModuleIdx::from_usize(cache.module_table.modules.len());

        if module_has_tla(&new_module) {
          cache.tla_module_count += 1;
        }
        if let Some(span) = incoming_tla_span {
          cache.tla_keyword_span_map.insert(new_module_idx, span);
        }
        cache.symbol_ref_db.store_local_db(
          new_module_idx,
          std::mem::take(scan_stage_output.symbol_ref_db.local_db_mut(new_idx)),
        );
        cache.module_table.modules.push(new_module);
        cache.index_ecma_ast.push(scan_stage_output.index_ecma_ast.get_mut(new_idx).take());
        cache.stmt_infos.push(std::mem::replace(
          scan_stage_output.stmt_infos.get_mut(new_idx),
          rolldown_common::StmtInfos::new(),
        ));
        continue;
      }
      let old_has_tla = module_has_tla(&cache.module_table[idx]);
      let new_has_tla = module_has_tla(&new_module);
      if old_has_tla && !new_has_tla {
        debug_assert!(
          cache.tla_module_count > 0,
          "tla_module_count underflow: decrement called when count is already 0"
        );
        cache.tla_module_count -= 1;
      } else if !old_has_tla && new_has_tla {
        cache.tla_module_count += 1;
      }
      match incoming_tla_span {
        Some(span) => {
          cache.tla_keyword_span_map.insert(idx, span);
        }
        None => {
          cache.tla_keyword_span_map.remove(&idx);
        }
      }
      cache.module_table[idx] = new_module;
      cache.index_ecma_ast[idx] = scan_stage_output.index_ecma_ast.get_mut(new_idx).take();
      cache.stmt_infos[idx] = std::mem::replace(
        scan_stage_output.stmt_infos.get_mut(new_idx),
        rolldown_common::StmtInfos::new(),
      );
      std::mem::swap(
        cache.symbol_ref_db.local_db_mut(idx),
        scan_stage_output.symbol_ref_db.local_db_mut(new_idx),
      );
    }

    // The scan rebuilds the materialized importer sets only for the modules
    // it re-scanned. Re-derive them for cached modules whose incoming edges
    // changed. See internal-docs/cache/implementation.md.
    let modules_with_changed_importers = std::mem::take(&mut self.modules_with_changed_importers);
    for idx in &modules_with_changed_importers {
      // The idx may belong to a module from a scan that failed before merging.
      let Some(module) = cache.module_table.modules.get_mut(*idx).and_then(Module::as_normal_mut)
      else {
        continue;
      };
      module.ecma_view.rebuild_importer_sets(&self.importers[*idx]);
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
          .extract_if(.., |(module_idx, _stmt_info_idx, _node_id, _)| {
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

    // Recompute user-defined entry modules for this build instead of monotonically extending.
    // `scan_stage_output.user_defined_entry_modules` only contains entries discovered in the
    // current scan (e.g. changed modules + emitted entries), so we additionally keep configured
    // root entries that remain valid in cache.
    let mut user_defined_entry_modules = scan_stage_output.user_defined_entry_modules;
    for user_defined_entry_id in &self.user_defined_entry {
      let Some(visit_state) = self.module_id_to_idx.get(user_defined_entry_id) else {
        continue;
      };
      let idx = visit_state.idx();
      if cache.module_table.modules.get(idx).is_some() {
        user_defined_entry_modules.insert(idx);
      }
    }
    // Entries emitted via `emitFile(type: 'chunk')` are only discovered by
    // full scans (partial scans skip `buildStart`); their rows persist in
    // `entry_points`, so keep their modules flagged as entries too.
    for entry in &cache.entry_points {
      if matches!(entry.kind, rolldown_common::EntryPointKind::EmittedUserDefined) {
        user_defined_entry_modules.insert(entry.idx);
      }
    }
    cache.user_defined_entry_modules = user_defined_entry_modules;

    // Keep the plugin-facing `ModuleInfo.importers` of those modules in
    // sync as well; the scan refreshes it only for re-scanned modules.
    for idx in modules_with_changed_importers {
      let Some(module) = cache.module_table.modules.get(idx).and_then(Module::as_normal) else {
        continue;
      };
      plugin_driver.set_module_info(
        &module.id,
        Arc::new(module.to_module_info(None, cache.user_defined_entry_modules.contains(&idx))),
      );
    }

    Ok(())
  }

  fn build_module_index_maps(&mut self, build_snapshot: &NormalizedScanStageOutput) {
    self.module_idx_by_abs_path.clear();
    self.module_idx_by_stable_id.clear();

    for module in &build_snapshot.module_table.modules {
      if let rolldown_common::Module::Normal(normal_module) = module {
        let filename = ArcStr::from(normal_module.id.as_arc_str().to_slash());
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
      stmt_infos: cache.stmt_infos.clone(),

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
      user_defined_entry_modules: cache.user_defined_entry_modules.clone(),
      tla_module_count: cache.tla_module_count,
      tla_keyword_span_map: cache.tla_keyword_span_map.clone(),
    }
  }
}
