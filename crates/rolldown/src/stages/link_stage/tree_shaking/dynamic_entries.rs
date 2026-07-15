//! Dynamic-entry handling for the inclusion fixpoint: topological ordering of dynamic
//! entries (so ancestors include before descendants), aliveness determination (a dead
//! pure dynamic import must not retain its entry), and per-iteration processing.

use std::cmp::Reverse;

use petgraph::prelude::DiGraphMap;
use rolldown_common::{
  EntryPoint, EntryPointKind, ImportKind, ImportRecordIdx, ImportRecordMeta, Module, ModuleIdx,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::stages::link_stage::{
  LinkStage,
  passes::{EntryExportRoots, UnreachableDynamicImports},
};

use super::include_statements::{
  IncludeContext, StmtInclusionVec, SymbolIncludeReason, include_declaring_statements,
  include_module, include_symbol_and_check_cjs_bailout,
};

impl LinkStage<'_> {
  /// Process a dynamic entry and determine if it should be retained.
  /// Returns `true` if the entry should be kept, `false` if it should be filtered out.
  pub(super) fn process_and_retain_dynamic_entry(
    &self,
    entry: &EntryPoint,
    cycled_idx: &FxHashSet<ModuleIdx>,
    context: &mut IncludeContext,
    unused_record_idxs: &mut Vec<(ModuleIdx, ImportRecordIdx)>,
    unreachable_import_expression_node_ids: &UnreachableDynamicImports,
    entry_export_roots: &EntryExportRoots,
  ) -> bool {
    if !cycled_idx.contains(&entry.idx) {
      if let Some(item) = self.is_dynamic_entry_alive(
        entry,
        context.is_included_vec,
        unreachable_import_expression_node_ids,
      ) {
        unused_record_idxs.extend(item);
        return false;
      }
    }
    let module = match &self.module_table[entry.idx] {
      Module::Normal(module) => module,
      Module::External(_module) => {
        // Case: import('external').
        return true;
      }
    };
    entry_export_roots.get(entry.idx).unwrap_or_default().iter().for_each(|root| {
      let symbol_ref = root.symbol_ref;
      if let Module::Normal(_) = &context.modules[symbol_ref.owner] {
        include_declaring_statements(context, &symbol_ref);
        include_symbol_and_check_cjs_bailout(context, symbol_ref, SymbolIncludeReason::EntryExport);
      }
    });
    include_module(context, module);
    true
  }

  /// # Description
  /// Some dynamic entries also reference another dynamic entry, we need to ensure each
  /// dynamic entry is included before all its descendant dynamic entry.
  /// ```js
  /// // a.js
  /// export default import('./b.js').then((mod) => {
  ///   return mod;
  /// })
  ///
  /// // b.js
  /// export default import('./c.js').then((mod) => {
  ///  return mod;
  /// })
  ///
  /// // c.js
  /// export default 1;
  /// ```
  /// after first round user defined entry are included, `default` of `b.js` are included, but
  /// `default` of `c.js` is not included.
  /// note: We can't use default entry point order, since they are sorted by stable_id.
  ///
  /// # Complexity
  ///   - construct the dynamic entry relation graph: O(M), `M` the number of modules.
  ///   - ref https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm#Complexity
  ///     `O(|V|+|E|)`, for the most of the scenario the relation graph is sparsely connected, we
  ///     could assume it is `O(N)`, `N` is the number of dynamic entries.
  ///   - So overall, the complexity is `O(M)`.
  pub(super) fn sort_dynamic_entries_by_topological_order(
    &self,
    dynamic_entries: &mut [EntryPoint],
  ) -> FxHashSet<ModuleIdx> {
    let mut graph: DiGraphMap<ModuleIdx, ()> = DiGraphMap::new();

    // TODO: Since we don't skip visited node, If a project has a lot of dynamic entries,
    // and they are all connected, the performance may be impacted. But this seems rare in real world,
    // we could optimize it later if needed.
    for entry in dynamic_entries.iter() {
      let mut entry_module_idx = entry.idx;
      let cur = entry_module_idx;
      let mut visited = FxHashSet::default();
      self.construct_dynamic_entry_graph(&mut graph, &mut visited, &mut entry_module_idx, cur);
    }
    let mut cycled_dynamic_entries = FxHashSet::default();
    // https://docs.rs/petgraph/latest/petgraph/algo/fn.tarjan_scc.html
    // the order of struct connected component is sorted by reverse topological sort.
    let idx_to_order_map = petgraph::algo::tarjan_scc(&graph)
      .into_iter()
      .enumerate()
      .filter(|(_idx, scc)| {
        if scc.len() > 1 {
          cycled_dynamic_entries.extend(scc.iter().copied());
          return false;
        }
        true
      })
      .map(|(idx, scc)| (scc[0], idx))
      .collect::<FxHashMap<ModuleIdx, usize>>();
    // We only need to ensure the relative order of those none cycled dynamic entries are correct, rest of them
    // we just bailout them
    dynamic_entries.sort_by_key(|item| {
      idx_to_order_map.get(&item.idx).map_or(Reverse(usize::MAX), |&order| Reverse(order))
    });
    cycled_dynamic_entries
  }

  fn construct_dynamic_entry_graph(
    &self,
    g: &mut DiGraphMap<ModuleIdx, ()>,
    visited: &mut FxHashSet<ModuleIdx>,
    root_node: &mut ModuleIdx,
    cur_node: ModuleIdx,
  ) -> Option<()> {
    if visited.contains(&cur_node) {
      return Some(());
    }
    visited.insert(cur_node);
    let module = self.module_table[cur_node].as_normal()?;
    for rec in &module.import_records {
      let Some(module_idx) = rec.resolved_module else {
        continue;
      };
      if rec.kind == ImportKind::DynamicImport {
        let seen = g.contains_node(module_idx);
        if *root_node != module_idx {
          g.add_edge(*root_node, module_idx, ());
          // Even it is visited before, we still needs to connect the edge
          if seen {
            continue;
          }
        }
        let previous = *root_node;
        *root_node = module_idx;
        self.construct_dynamic_entry_graph(g, visited, root_node, module_idx);
        *root_node = previous;
        continue;
      }
      // Can't put it at the beginning of the loop,
      self.construct_dynamic_entry_graph(g, visited, root_node, module_idx);
    }
    Some(())
  }

  /// Note:
  /// this function determine if a dynamic_entry is still alive, return the unused dynamic
  /// import record idxs(due to limitation of rustc borrow checker) if it is unused.
  fn is_dynamic_entry_alive(
    &self,
    entry_point: &EntryPoint,
    is_stmt_included_vec: &StmtInclusionVec,
    unreachable_import_expression_node_ids: &UnreachableDynamicImports,
  ) -> Option<Vec<(ModuleIdx, ImportRecordIdx)>> {
    let mut ret = vec![];
    let is_lived = match entry_point.kind {
      EntryPointKind::UserDefined | EntryPointKind::EmittedUserDefined => true,
      EntryPointKind::DynamicImport => {
        let is_dynamic_imported_module_exports_unused =
          self.dynamic_import_exports_usage_map.get(&entry_point.idx).is_some_and(
            |item| matches!(item, DynamicImportExportsUsage::Partial(set) if set.is_empty()),
          );

        // Mark the dynamic entry as lived if at least one statement that create this entry is included
        entry_point.related_stmt_infos.iter().any(
          |(module_idx, stmt_idx, node_id, import_record_idx)| {
            if unreachable_import_expression_node_ids.contains(*module_idx, *node_id) {
              return false;
            }
            let module =
              &self.module_table[*module_idx].as_normal().expect("should be a normal module");
            let all_dead_pure_dynamic_import = {
              let import_record = &module.import_records[*import_record_idx];
              let importee_side_effects = self.module_table[import_record.into_resolved_module()]
                .side_effects()
                .has_side_effects();

              // Only consider it is unused if it is a top level pure dynamic import and the
              // importee module has no side effects.
              !importee_side_effects
                && import_record.meta.contains(ImportRecordMeta::TopLevelPureDynamicImport)
            };
            let is_stmt_included = is_stmt_included_vec[*module_idx].has_bit(*stmt_idx);
            let lived = is_stmt_included
              && (!is_dynamic_imported_module_exports_unused || !all_dead_pure_dynamic_import);

            if !lived && all_dead_pure_dynamic_import {
              ret.push((*module_idx, *import_record_idx));
            }
            lived
          },
        )
      }
    };
    (!is_lived).then_some(ret)
  }
}
