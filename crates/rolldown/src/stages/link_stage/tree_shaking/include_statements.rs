use std::cmp::Reverse;

// cSpell:ignore idxs
use itertools::Itertools;
use oxc_index::IndexVec;
use petgraph::prelude::DiGraphMap;
use rolldown_common::{
  EcmaViewMeta, EntryPoint, EntryPointKind, ExportsKind, ImportKind, ImportRecordIdx,
  ImportRecordMeta, IndexModules, Module, ModuleIdx, ModuleType, NormalModule,
  NormalizedBundlerOptions, StmtInfoIdx, SymbolOrMemberExprRef, SymbolRef, SymbolRefDb,
  dynamic_import_usage::DynamicImportExportsUsage, side_effects::DeterminedSideEffects,
};
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{stages::link_stage::LinkStage, types::linking_metadata::LinkingMetadataVec};

struct Context<'a> {
  modules: &'a IndexModules,
  symbols: &'a SymbolRefDb,
  is_included_vec: &'a mut IndexVec<ModuleIdx, IndexVec<StmtInfoIdx, bool>>,
  is_module_included_vec: &'a mut IndexVec<ModuleIdx, bool>,
  tree_shaking: bool,
  runtime_id: ModuleIdx,
  metas: &'a LinkingMetadataVec,
  used_symbol_refs: &'a mut FxHashSet<SymbolRef>,
  options: &'a NormalizedBundlerOptions,
  normal_symbol_exports_chain_map: &'a FxHashMap<SymbolRef, Vec<SymbolRef>>,
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn include_statements(&mut self) {
    let mut is_included_vec: IndexVec<ModuleIdx, IndexVec<StmtInfoIdx, bool>> = self
      .module_table
      .modules
      .iter()
      .map(|m| {
        m.as_normal().map_or(IndexVec::default(), |m| {
          m.stmt_infos.iter().map(|_| false).collect::<IndexVec<StmtInfoIdx, _>>()
        })
      })
      .collect::<IndexVec<ModuleIdx, _>>();
    let mut used_symbol_refs = FxHashSet::default();
    let mut is_module_included_vec: IndexVec<ModuleIdx, bool> =
      oxc_index::index_vec![false; self.module_table.modules.len()];

    let context = &mut Context {
      modules: &self.module_table.modules,
      symbols: &self.symbols,
      is_included_vec: &mut is_included_vec,
      is_module_included_vec: &mut is_module_included_vec,
      tree_shaking: self.options.treeshake.is_some(),
      runtime_id: self.runtime.id(),
      // used_exports_info_vec: &mut used_exports_info_vec,
      metas: &self.metas,
      used_symbol_refs: &mut used_symbol_refs,
      options: self.options,
      normal_symbol_exports_chain_map: &self.normal_symbol_exports_chain_map,
    };

    let (user_defined_entries, mut dynamic_entries): (Vec<_>, Vec<_>) =
      std::mem::take(&mut self.entries)
        .into_iter()
        .partition(|item| matches!(item.kind, EntryPointKind::UserDefined));
    user_defined_entries
      .iter()
      .filter(|entry| matches!(entry.kind, EntryPointKind::UserDefined))
      .for_each(|entry| {
        let module = match &self.module_table.modules[entry.id] {
          Module::Normal(module) => module,
          Module::External(_module) => {
            // Case: import('external').
            return;
          }
        };
        let meta = &self.metas[entry.id];
        meta.referenced_symbols_by_entry_point_chunk.iter().for_each(|symbol_ref| {
          include_symbol(context, *symbol_ref);
        });
        include_module(context, module);
      });

    if self.options.is_hmr_enabled() {
      // HMR runtime contains statements with side effects, they are referenced by other modules via global variables.
      // So we need to manually include them here.
      if let Some(runtime_module) = self.module_table.modules[self.runtime.id()].as_normal() {
        runtime_module.stmt_infos.iter_enumerated().for_each(|(stmt_info_id, stmt_info)| {
          if stmt_info.side_effect {
            include_statement(context, runtime_module, stmt_info_id);
          }
        });
      }
    }

    let mut unused_record_idxs = vec![];
    let cycled_idx = self.sort_dynamic_entries_by_topological_order(&mut dynamic_entries);

    dynamic_entries.retain(|entry| {
      if !cycled_idx.contains(&entry.id) {
        if let Some(item) = self.is_dynamic_entry_alive(entry, context.is_included_vec) {
          unused_record_idxs.extend(item);
          return false;
        }
      }
      let module = match &self.module_table.modules[entry.id] {
        Module::Normal(module) => module,
        Module::External(_module) => {
          // Case: import('external').
          return true;
        }
      };
      let meta = &self.metas[entry.id];
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(|symbol_ref| {
        include_symbol(context, *symbol_ref);
      });
      include_module(context, module);
      true
    });

    // update entries with lived only.
    self.entries = user_defined_entries.into_iter().chain(dynamic_entries).collect();

    // mark those dynamic import records as dead, in case we could eliminate them later in ast
    // visitor.
    for (mi, record_idxs) in unused_record_idxs {
      let module =
        self.module_table.modules[mi].as_normal_mut().expect("should be a normal module");
      for record_idx in record_idxs {
        let rec = &mut module.import_records[record_idx];
        rec.meta.insert(ImportRecordMeta::DEAD_DYNAMIC_IMPORT);
      }
    }

    self.module_table.modules.par_iter_mut().filter_map(Module::as_normal_mut).for_each(|module| {
      let idx = module.idx;
      module.meta.set(EcmaViewMeta::INCLUDED, is_module_included_vec[idx]);
      is_included_vec[module.idx].iter_enumerated().for_each(|(stmt_info_id, is_included)| {
        module.stmt_infos.get_mut(stmt_info_id).is_included = *is_included;
      });

      // The hmr need to create module namespace object to store exports.
      if self.options.is_hmr_enabled()
        && module.idx != self.runtime.id()
        && matches!(module.exports_kind, ExportsKind::Esm)
      {
        module.stmt_infos.get_mut(StmtInfoIdx::new(0)).is_included = true;
      }
    });

    self.used_symbol_refs = used_symbol_refs;

    tracing::trace!(
      "included statements {:#?}",
      self
        .module_table
        .modules
        .iter()
        .filter_map(Module::as_normal)
        .map(NormalModule::to_debug_normal_module_for_tree_shaking)
        .collect::<Vec<_>>()
    );
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
  fn sort_dynamic_entries_by_topological_order(
    &self,
    dynamic_entries: &mut [EntryPoint],
  ) -> FxHashSet<ModuleIdx> {
    let mut graph: DiGraphMap<ModuleIdx, ()> = DiGraphMap::new();
    for entry in dynamic_entries.iter() {
      let mut entry_module_idx = entry.id;
      let cur = entry_module_idx;
      if graph.contains_node(cur) {
        continue;
      }
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
      idx_to_order_map.get(&item.id).map_or(Reverse(usize::MAX), |&order| Reverse(order))
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
    let module = self.module_table.modules[cur_node].as_normal()?;
    for ele in &module.import_records {
      if ele.kind == ImportKind::DynamicImport {
        let seen = g.contains_node(ele.resolved_module);
        if *root_node != ele.resolved_module {
          g.add_edge(*root_node, ele.resolved_module, ());
          // Even it is visited before, we still needs to connect the edge
          if seen {
            continue;
          }
        }
        let previous = *root_node;
        *root_node = ele.resolved_module;
        self.construct_dynamic_entry_graph(g, visited, root_node, ele.resolved_module);
        *root_node = previous;
        continue;
      }
      // Can't put it at the beginning of the loop,
      self.construct_dynamic_entry_graph(g, visited, root_node, ele.resolved_module);
    }
    Some(())
  }

  /// Note:
  /// this function determine if a dynamic_entry is still alive, return the unused dynamic
  /// import record idxs(due to limination of rustc borrow checker) if it is unused.
  fn is_dynamic_entry_alive(
    &self,
    item: &EntryPoint,
    is_stmt_included_vec: &IndexVec<ModuleIdx, IndexVec<StmtInfoIdx, bool>>,
  ) -> Option<Vec<(ModuleIdx, Vec<ImportRecordIdx>)>> {
    let mut ret = vec![];
    let is_lived = match item.kind {
      EntryPointKind::UserDefined => true,
      EntryPointKind::DynamicImport => {
        let is_dynamic_imported_module_exports_unused =
          self.dynamic_import_exports_usage_map.get(&item.id).is_some_and(
            |item| matches!(item, DynamicImportExportsUsage::Partial(set) if set.is_empty()),
          );

        // Mark the dynamic entry as lived if at least one statement that create this entry is included
        item.related_stmt_infos.iter().any(|(module_idx, stmt_idx)| {
          let module =
            &self.module_table.modules[*module_idx].as_normal().expect("should be a normal module");
          let stmt_info = &module.stmt_infos[*stmt_idx];
          let mut dead_pure_dynamic_import_record_idx = vec![];
          let all_dead_pure_dynamic_import =
            stmt_info.import_records.iter().all(|import_record_idx| {
              let import_record = &module.import_records[*import_record_idx];
              if import_record.resolved_module.is_dummy() {
                return true;
              }
              let importee_side_effects = self.module_table.modules[import_record.resolved_module]
                .side_effects()
                .has_side_effects();
              let ret = !importee_side_effects;
              if ret {
                dead_pure_dynamic_import_record_idx.push(*import_record_idx);
              }
              ret
            });
          let is_stmt_included = is_stmt_included_vec[*module_idx][*stmt_idx];
          let lived = is_stmt_included
            && (!is_dynamic_imported_module_exports_unused || !all_dead_pure_dynamic_import);

          if !lived {
            ret.push((*module_idx, dead_pure_dynamic_import_record_idx));
          }
          lived
        })
      }
    };
    if is_lived { None } else { Some(ret) }
  }
}

/// if no export is used, and the module has no side effects, the module should not be included
fn include_module(ctx: &mut Context, module: &NormalModule) {
  if ctx.is_module_included_vec[module.idx] {
    return;
  }

  ctx.is_module_included_vec[module.idx] = true;

  if module.idx == ctx.runtime_id && !ctx.options.is_hmr_enabled() {
    // runtime module has no side effects and it's statements should be included
    // by other modules's references.
    return;
  }

  let forced_no_treeshake = matches!(module.side_effects, DeterminedSideEffects::NoTreeshake);
  if ctx.tree_shaking && !forced_no_treeshake {
    module.stmt_infos.iter_enumerated().for_each(|(stmt_info_id, stmt_info)| {
      // No need to handle the first statement specially, which is the namespace object, because it doesn't have side effects and will only be included if it is used.
      let bail_eval = module.meta.has_eval()
        && !stmt_info.declared_symbols.is_empty()
        && stmt_info_id.index() != 0;
      if stmt_info.side_effect || bail_eval {
        include_statement(ctx, module, stmt_info_id);
      }
    });
  } else {
    // Skip the first statement, which is the namespace object. It should be included only if it is used no matter
    // tree shaking is enabled or not.
    module.stmt_infos.iter_enumerated().skip(1).for_each(|(stmt_info_id, stmt_info)| {
      if stmt_info.force_tree_shaking {
        if stmt_info.side_effect {
          // If `force_tree_shaking` is true, the statement should be included either by itself having side effects
          // or by other statements referencing it.
          include_statement(ctx, module, stmt_info_id);
        }
      } else {
        include_statement(ctx, module, stmt_info_id);
      }
    });
  }

  let module_meta = &ctx.metas[module.idx];

  // Include imported modules for its side effects
  module_meta.dependencies.iter().copied().for_each(|dependency_idx| {
    match &ctx.modules[dependency_idx] {
      Module::Normal(importee) => {
        if !ctx.tree_shaking || importee.side_effects.has_side_effects() {
          include_module(ctx, importee);
        }
      }
      Module::External(_) => {}
    }
  });
  tracing::trace!(
    "{}:\n module_meta dependencies: {:#?}",
    module.stable_id,
    module_meta.dependencies.iter().map(|idx| { ctx.modules[*idx].id().to_string() }).collect_vec()
  );
  if module.meta.has_eval() && matches!(module.module_type, ModuleType::Js | ModuleType::Jsx) {
    module.named_imports.keys().for_each(|symbol| {
      include_symbol(ctx, *symbol);
    });
  }
}

fn include_symbol(ctx: &mut Context, symbol_ref: SymbolRef) {
  let mut canonical_ref = ctx.symbols.canonical_ref_for(symbol_ref);
  let canonical_ref_symbol = ctx.symbols.get(canonical_ref);
  if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
    canonical_ref = namespace_alias.namespace_ref;
  }

  ctx.used_symbol_refs.insert(canonical_ref);

  if let Module::Normal(module) = &ctx.modules[canonical_ref.owner] {
    include_module(ctx, module);
    module.stmt_infos.declared_stmts_by_symbol(&canonical_ref).iter().copied().for_each(
      |stmt_info_id| {
        include_statement(ctx, module, stmt_info_id);
      },
    );
  }
}

fn include_statement(ctx: &mut Context, module: &NormalModule, stmt_info_id: StmtInfoIdx) {
  let is_included = &mut ctx.is_included_vec[module.idx][stmt_info_id];

  if *is_included {
    return;
  }

  let stmt_info = module.stmt_infos.get(stmt_info_id);

  // include the statement itself
  *is_included = true;

  stmt_info.referenced_symbols.iter().for_each(|reference_ref| {
    let original_ref = reference_ref.symbol_ref();
    std::iter::once(original_ref)
      .chain(
        ctx.normal_symbol_exports_chain_map.get(original_ref).map(Vec::as_slice).unwrap_or(&[]),
      )
      .for_each(|sym_ref| {
        if let Module::Normal(module) = &ctx.modules[sym_ref.owner] {
          module.stmt_infos.declared_stmts_by_symbol(sym_ref).iter().copied().for_each(
            |stmt_info_id| {
              include_statement(ctx, module, stmt_info_id);
            },
          );
        }
      });

    match reference_ref {
      SymbolOrMemberExprRef::Symbol(symbol_ref) => {
        include_symbol(ctx, *symbol_ref);
      }
      SymbolOrMemberExprRef::MemberExpr(member_expr) => {
        if let Some(symbol) =
          member_expr.resolved_symbol_ref(&ctx.metas[module.idx].resolved_member_expr_refs)
        {
          if let Some(resolution) =
            ctx.metas[module.idx].resolved_member_expr_refs.get(&member_expr.span)
          {
            resolution.depended_refs.iter().for_each(|sym_ref| {
              if let Module::Normal(module) = &ctx.modules[sym_ref.owner] {
                module.stmt_infos.declared_stmts_by_symbol(sym_ref).iter().copied().for_each(
                  |stmt_info_id| {
                    include_statement(ctx, module, stmt_info_id);
                  },
                );
              }
            });
          }
          include_symbol(ctx, symbol);
        }
      }
    }
  });
}
