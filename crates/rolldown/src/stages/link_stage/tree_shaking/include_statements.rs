use std::cmp::Reverse;

use itertools::Itertools;
use oxc_index::IndexVec;
use petgraph::prelude::DiGraphMap;
use rolldown_common::{
  ConstExportMeta, EcmaModuleAstUsage, EcmaViewMeta, EntryPoint, EntryPointKind, ExportsKind,
  ImportKind, ImportRecordIdx, ImportRecordMeta, IndexModules, Module, ModuleIdx,
  ModuleNamespaceIncludedReason, ModuleType, NormalModule, NormalizedBundlerOptions,
  RUNTIME_HELPER_NAMES, RuntimeHelper, SideEffectDetail, StmtInfoIdx, StmtInfoMeta, StmtInfos,
  SymbolIdExt, SymbolOrMemberExprRef, SymbolRef, SymbolRefDb,
  dynamic_import_usage::DynamicImportExportsUsage, side_effects::DeterminedSideEffects,
};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::rayon::{
  IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{stages::link_stage::LinkStage, types::linking_metadata::LinkingMetadataVec};

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    struct SymbolIncludeReason: u8 {
        const Normal = 1;
        const EntryExport = 1 << 1;
        /// See `has_dynamic_exports` in [`crate::types::linking_metadata::LinkingMetadata`]
        /// 1. https://github.com/rolldown/rolldown/blob/8bc7dca5a09047b6b494e3fa7b6b7564aa465372/crates/rolldown/src/stages/link_stage/reference_needed_symbols.rs?plain=1#L122-L134
        /// 2. https://github.com/rolldown/rolldown/blob/8bc7dca5a09047b6b494e3fa7b6b7564aa465372/crates/rolldown/src/stages/link_stage/reference_needed_symbols.rs?plain=1#L188-L197
        const ReExportDynamicExports = 1 << 2;
    }
}

/// [SymbolIncludeReason]
struct Context<'a> {
  modules: &'a IndexModules,
  symbols: &'a SymbolRefDb,
  is_included_vec: &'a mut IndexVec<ModuleIdx, IndexVec<StmtInfoIdx, bool>>,
  is_module_included_vec: &'a mut IndexVec<ModuleIdx, bool>,
  tree_shaking: bool,
  inline_const_smart: bool,
  runtime_id: ModuleIdx,
  metas: &'a LinkingMetadataVec,
  used_symbol_refs: &'a mut FxHashSet<SymbolRef>,
  constant_symbol_map: &'a FxHashMap<SymbolRef, ConstExportMeta>,
  options: &'a NormalizedBundlerOptions,
  normal_symbol_exports_chain_map: &'a FxHashMap<SymbolRef, Vec<SymbolRef>>,
  /// It is necessary since we can't mutate `module.meta` during the tree shaking process.
  /// see [rolldown_common::ecmascript::ecma_view::EcmaViewMeta]
  bailout_cjs_tree_shaking_modules: FxHashSet<ModuleIdx>,
  may_partial_namespace: bool,
  module_namespace_included_reason: &'a mut IndexVec<ModuleIdx, ModuleNamespaceIncludedReason>,
}

impl LinkStage<'_> {
  #[allow(clippy::too_many_lines)]
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
    let mut module_namespace_included_reason: IndexVec<ModuleIdx, ModuleNamespaceIncludedReason> =
      oxc_index::index_vec![ModuleNamespaceIncludedReason::empty(); self.module_table.len()];
    let context = &mut Context {
      modules: &self.module_table.modules,
      symbols: &self.symbols,
      is_included_vec: &mut is_included_vec,
      is_module_included_vec: &mut is_module_included_vec,
      tree_shaking: self.options.treeshake.is_some(),
      runtime_id: self.runtime.id(),
      metas: &self.metas,
      used_symbol_refs: &mut used_symbol_refs,
      constant_symbol_map: &self.constant_symbol_map,
      options: self.options,
      normal_symbol_exports_chain_map: &self.normal_symbol_exports_chain_map,
      bailout_cjs_tree_shaking_modules: FxHashSet::default(),
      may_partial_namespace: false,
      module_namespace_included_reason: &mut module_namespace_included_reason,
      inline_const_smart: self.options.optimization.is_inline_const_smart_mode(),
    };

    let (user_defined_entries, mut dynamic_entries): (Vec<_>, Vec<_>) =
      std::mem::take(&mut self.entries).into_iter().partition(|item| item.kind.is_user_defined());
    user_defined_entries.iter().filter(|entry| entry.kind.is_user_defined()).for_each(|entry| {
      let module = match &self.module_table[entry.idx] {
        Module::Normal(module) => module,
        Module::External(_module) => {
          // Case: import('external').
          return;
        }
      };
      context.bailout_cjs_tree_shaking_modules.insert(module.idx);
      let meta = &self.metas[entry.idx];
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(
        |(symbol_ref, _came_from_cjs)| {
          if let Module::Normal(module) = &context.modules[symbol_ref.owner] {
            module.stmt_infos.declared_stmts_by_symbol(symbol_ref).iter().copied().for_each(
              |stmt_info_id| {
                include_statement(context, module, stmt_info_id);
              },
            );
            include_symbol(context, *symbol_ref, SymbolIncludeReason::EntryExport);
          }
        },
      );
      include_module(context, module);
    });

    let mut unused_record_idxs = vec![];
    let cycled_idx = self.sort_dynamic_entries_by_topological_order(&mut dynamic_entries);

    dynamic_entries.retain(|entry| {
      if !cycled_idx.contains(&entry.idx) {
        if let Some(item) = self.is_dynamic_entry_alive(entry, context.is_included_vec) {
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
      let meta = &self.metas[entry.idx];
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(
        |(symbol_ref, _came_from_cjs)| {
          if let Module::Normal(module) = &context.modules[symbol_ref.owner] {
            module.stmt_infos.declared_stmts_by_symbol(symbol_ref).iter().copied().for_each(
              |stmt_info_id| {
                include_statement(context, module, stmt_info_id);
              },
            );
            include_symbol(context, *symbol_ref, SymbolIncludeReason::EntryExport);
          }
        },
      );
      include_module(context, module);
      true
    });

    // update entries with lived only.
    self.entries = user_defined_entries.into_iter().chain(dynamic_entries).collect();

    // It could be safely take since it is no more used.
    for idx in std::mem::take(&mut context.bailout_cjs_tree_shaking_modules) {
      self.metas[idx]
        .resolved_exports
        .iter()
        .filter_map(|(_name, local)| local.came_from_cjs.then_some(local))
        .for_each(|local| {
          include_symbol(context, local.symbol_ref, SymbolIncludeReason::Normal);
        });
    }

    // mark those dynamic import records as dead, in case we could eliminate them later in ast
    // visitor.
    for (mi, record_idxs) in unused_record_idxs {
      let module = self.module_table[mi].as_normal_mut().expect("should be a normal module");
      for record_idx in record_idxs {
        let rec = &mut module.import_records[record_idx];
        rec.meta.insert(ImportRecordMeta::DeadDynamicImport);
      }
    }

    self
      .module_table
      .modules
      .par_iter_mut()
      .zip_eq(self.metas.par_iter_mut())
      .filter_map(|(m, meta)| m.as_normal_mut().map(|m| (m, meta)))
      .for_each(|(module, meta)| {
        let idx = module.idx;
        module.meta.set(EcmaViewMeta::Included, is_module_included_vec[idx]);
        is_included_vec[module.idx].iter_enumerated().for_each(|(stmt_info_id, is_included)| {
          module.stmt_infos.get_mut(stmt_info_id).is_included = *is_included;
        });
        let mut normalized_runtime_helper = RuntimeHelper::default();
        for (index, stmt_info_idxs) in module.depended_runtime_helper.iter().enumerate() {
          if stmt_info_idxs.is_empty() {
            continue;
          }
          let any_included =
            stmt_info_idxs.iter().any(|stmt_info_idx| is_included_vec[module.idx][*stmt_info_idx]);
          #[allow(clippy::cast_possible_truncation)]
          // It is alright, since the `RuntimeHelper` is a bitmask and the index is guaranteed to be less than 32.
          normalized_runtime_helper
            .set(RuntimeHelper::from_bits(1 << index as u32).unwrap(), any_included);
        }
        meta.depended_runtime_helper = normalized_runtime_helper;
        meta.module_namespace_included_reason = module_namespace_included_reason[module.idx];
      });

    self.include_runtime_symbol(
      &mut is_included_vec,
      &mut is_module_included_vec,
      &mut module_namespace_included_reason,
      &mut used_symbol_refs,
    );
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
      let mut entry_module_idx = entry.idx;
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
      if rec.kind == ImportKind::DynamicImport {
        let seen = g.contains_node(rec.resolved_module);
        if *root_node != rec.resolved_module {
          g.add_edge(*root_node, rec.resolved_module, ());
          // Even it is visited before, we still needs to connect the edge
          if seen {
            continue;
          }
        }
        let previous = *root_node;
        *root_node = rec.resolved_module;
        self.construct_dynamic_entry_graph(g, visited, root_node, rec.resolved_module);
        *root_node = previous;
        continue;
      }
      // Can't put it at the beginning of the loop,
      self.construct_dynamic_entry_graph(g, visited, root_node, rec.resolved_module);
    }
    Some(())
  }

  /// Note:
  /// this function determine if a dynamic_entry is still alive, return the unused dynamic
  /// import record idxs(due to limitation of rustc borrow checker) if it is unused.
  fn is_dynamic_entry_alive(
    &self,
    item: &EntryPoint,
    is_stmt_included_vec: &IndexVec<ModuleIdx, IndexVec<StmtInfoIdx, bool>>,
  ) -> Option<Vec<(ModuleIdx, Vec<ImportRecordIdx>)>> {
    let mut ret = vec![];
    let is_lived = match item.kind {
      EntryPointKind::UserDefined | EntryPointKind::EmittedUserDefined => true,
      EntryPointKind::DynamicImport => {
        let is_dynamic_imported_module_exports_unused =
          self.dynamic_import_exports_usage_map.get(&item.idx).is_some_and(
            |item| matches!(item, DynamicImportExportsUsage::Partial(set) if set.is_empty()),
          );

        // Mark the dynamic entry as lived if at least one statement that create this entry is included
        item.related_stmt_infos.iter().any(|(module_idx, stmt_idx)| {
          let module =
            &self.module_table[*module_idx].as_normal().expect("should be a normal module");
          let stmt_info = &module.stmt_infos[*stmt_idx];
          let mut dead_pure_dynamic_import_record_idx = vec![];
          let all_dead_pure_dynamic_import =
            stmt_info.import_records.iter().all(|import_record_idx| {
              let import_record = &module.import_records[*import_record_idx];
              let importee_side_effects =
                self.module_table[import_record.resolved_module].side_effects().has_side_effects();

              let ret = !importee_side_effects
                && import_record.meta.contains(ImportRecordMeta::TopLevelPureDynamicImport);

              // Only consider it is unused if it is a top level pure dynamic import and the
              // importee module has no side effects.
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
    (!is_lived).then_some(ret)
  }

  fn include_runtime_symbol(
    &mut self,
    is_stmt_included_vec: &mut IndexVec<ModuleIdx, IndexVec<StmtInfoIdx, bool>>,
    is_module_included_vec: &mut IndexVec<ModuleIdx, bool>,
    module_namespace_included_reason: &mut IndexVec<ModuleIdx, ModuleNamespaceIncludedReason>,
    used_symbol_refs: &mut FxHashSet<SymbolRef>,
  ) {
    // Including all depended runtime symbol
    let iter = self.metas.par_iter().map(|item| item.depended_runtime_helper);

    #[cfg(not(target_family = "wasm"))]
    let depended_runtime_helper = iter.reduce(RuntimeHelper::default, |a, b| a | b);
    #[cfg(target_family = "wasm")]
    let depended_runtime_helper = iter.reduce(|a, b| a | b).unwrap_or_default();

    if depended_runtime_helper.is_empty() {
      return;
    }

    let context = &mut Context {
      modules: &self.module_table.modules,
      symbols: &self.symbols,
      is_included_vec: is_stmt_included_vec,
      is_module_included_vec,
      tree_shaking: self.options.treeshake.is_some(),
      runtime_id: self.runtime.id(),
      // used_exports_info_vec: &mut used_exports_info_vec,
      metas: &self.metas,
      used_symbol_refs,
      constant_symbol_map: &self.constant_symbol_map,
      options: self.options,
      normal_symbol_exports_chain_map: &self.normal_symbol_exports_chain_map,
      bailout_cjs_tree_shaking_modules: FxHashSet::default(),
      may_partial_namespace: false,
      module_namespace_included_reason,
      inline_const_smart: self.options.optimization.is_inline_const_smart_mode(),
    };

    for helper in depended_runtime_helper {
      let index = helper.bits().trailing_zeros() as usize;
      let name = RUNTIME_HELPER_NAMES[index];
      include_symbol(context, self.runtime.resolve_symbol(name), SymbolIncludeReason::Normal);
    }

    let module =
      self.module_table[self.runtime.id()].as_normal_mut().expect("should be a normal module");
    module.meta.set(EcmaViewMeta::Included, true);

    for (stmt_idx, included) in is_stmt_included_vec[self.runtime.id()].iter_enumerated() {
      module.stmt_infos.get_mut(stmt_idx).is_included = *included;
    }
  }
}

/// if no export is used, and the module has no side effects, the module should not be included
fn include_module(ctx: &mut Context, module: &NormalModule) {
  if ctx.is_module_included_vec[module.idx] {
    return;
  }

  ctx.is_module_included_vec[module.idx] = true;

  if module.idx == ctx.runtime_id {
    // runtime module has no side effects and it's statements should be included
    // by other modules's references.
    return;
  }

  let forced_no_treeshake = matches!(module.side_effects, DeterminedSideEffects::NoTreeshake);
  if ctx.tree_shaking && !forced_no_treeshake {
    module.stmt_infos.iter_enumerated_without_namespace_stmt().for_each(
      |(stmt_info_id, stmt_info)| {
        // No need to handle the namespace statement specially, because it doesn't have side effects and will only be included if it is used.
        let bail_eval = module.meta.has_eval()
          && !stmt_info.declared_symbols.is_empty()
          && stmt_info_id.index() != 0;
        let has_side_effects = if module.meta.contains(EcmaViewMeta::SafelyTreeshakeCommonjs)
          && ctx.options.treeshake.commonjs()
        {
          stmt_info.side_effect.contains(SideEffectDetail::Unknown)
        } else {
          stmt_info.side_effect.has_side_effect()
        };
        if has_side_effects || bail_eval {
          include_statement(ctx, module, stmt_info_id);
        }
      },
    );
  } else {
    // Skip the namespace statement. It should be included only if it is used no matter tree shaking is enabled or not.
    module.stmt_infos.iter_enumerated_without_namespace_stmt().for_each(
      |(stmt_info_id, stmt_info)| {
        if stmt_info.force_tree_shaking {
          if stmt_info.side_effect.has_side_effect() {
            // If `force_tree_shaking` is true, the statement should be included either by itself having side effects
            // or by other statements referencing it.
            include_statement(ctx, module, stmt_info_id);
          }
        } else {
          include_statement(ctx, module, stmt_info_id);
        }
      },
    );
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
      include_symbol(ctx, *symbol, SymbolIncludeReason::Normal);
    });
  }

  ctx.metas[module.idx].included_commonjs_export_symbol.iter().for_each(|symbol_ref| {
    include_symbol(ctx, *symbol_ref, SymbolIncludeReason::Normal);
  });

  // With enabling HMR, rolldown will register included esm module's namespace object to the runtime.
  if ctx.options.is_hmr_enabled()
    && module.idx != ctx.runtime_id
    && matches!(module.exports_kind, ExportsKind::Esm)
  {
    include_statement(ctx, module, StmtInfos::NAMESPACE_STMT_IDX);
    ctx.module_namespace_included_reason[module.idx].insert(ModuleNamespaceIncludedReason::Unknown);
  }
}

fn include_symbol(ctx: &mut Context, symbol_ref: SymbolRef, include_kind: SymbolIncludeReason) {
  let mut canonical_ref = ctx.symbols.canonical_ref_for(symbol_ref);

  if let Some(v) = ctx.constant_symbol_map.get(&canonical_ref)
    && !include_kind.contains(SymbolIncludeReason::EntryExport)
    && !ctx.inline_const_smart
    && !v.commonjs_export
  {
    // If the symbol is a constant value and it is not a commonjs module export , we don't need to include it since it would be always inline
    // We don't need to add anyflag since if `inlineConst` is disabled, the test expr will always
    // return `false`
    return;
  }

  // Also include the symbol that points to the canonical ref.
  ctx.used_symbol_refs.insert(symbol_ref);

  if !ctx.may_partial_namespace {
    if let Some(idx) =
      ctx.metas[canonical_ref.owner].import_record_ns_to_cjs_module.get(&canonical_ref)
    {
      ctx.bailout_cjs_tree_shaking_modules.insert(*idx);
    }
    if ctx.modules[canonical_ref.owner].as_normal().map(|m| m.namespace_object_ref)
      == Some(canonical_ref)
    {
      ctx.bailout_cjs_tree_shaking_modules.insert(canonical_ref.owner);
    }
  }

  let canonical_ref_symbol = ctx.symbols.get(canonical_ref);
  if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
    canonical_ref = namespace_alias.namespace_ref;
    if let Some(idx) =
      ctx.metas[canonical_ref.owner].import_record_ns_to_cjs_module.get(&canonical_ref)
    {
      if !ctx.may_partial_namespace && namespace_alias.property_name.as_str() == "default" {
        ctx.bailout_cjs_tree_shaking_modules.insert(*idx);
      } else {
        // handle case:
        // ```js
        // import {a} from './cjs.js'
        // console.log(a)
        // ```
        ctx.modules[*idx].as_normal().inspect(|_| {
          let Some(export_symbol) =
            ctx.metas[*idx].resolved_exports.get(&namespace_alias.property_name)
          else {
            return;
          };
          if namespace_alias.property_name.as_str() != "default" {
            include_symbol(ctx, export_symbol.symbol_ref, SymbolIncludeReason::Normal);
          }
        });
      }
    }
  }

  if canonical_ref.symbol.is_module_namespace() {
    if include_kind.intersects(SymbolIncludeReason::Normal | SymbolIncludeReason::EntryExport) {
      ctx.module_namespace_included_reason[canonical_ref.owner]
        .insert(ModuleNamespaceIncludedReason::Unknown);
    } else if include_kind.contains(SymbolIncludeReason::ReExportDynamicExports) {
      ctx.module_namespace_included_reason[canonical_ref.owner]
        .insert(ModuleNamespaceIncludedReason::ReExportExternalModule);
    }
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

  // FIXME: bailout for require() import for now
  // it is fine for now, since webpack did not support it either
  // ```js
  // const cjs = require('./cjs.js')
  // ```
  stmt_info.import_records.iter().for_each(|import_record_idx| {
    let import_record = &module.import_records[*import_record_idx];
    let module_idx = import_record.resolved_module;
    let Some(m) = ctx.modules[module_idx].as_normal() else {
      // If the import record is not a normal module, we don't need to include it.
      return;
    };
    if !matches!(m.exports_kind, ExportsKind::CommonJs) || import_record.kind == ImportKind::Import
    {
      return;
    }
    if !module.ast_usage.contains(EcmaModuleAstUsage::IsCjsReexport) {
      ctx.bailout_cjs_tree_shaking_modules.insert(module_idx);
    }
  });
  let include_kind = if stmt_info.meta.contains(StmtInfoMeta::ReExportDynamicExports) {
    SymbolIncludeReason::ReExportDynamicExports
  } else {
    SymbolIncludeReason::Normal
  };
  stmt_info.referenced_symbols.iter().for_each(|reference_ref| {
    if let Some(member_expr_resolution) = match reference_ref {
      SymbolOrMemberExprRef::Symbol(_) => None,
      SymbolOrMemberExprRef::MemberExpr(member_expr_ref) => {
        member_expr_ref.resolution(&ctx.metas[module.idx].resolved_member_expr_refs)
      }
    } {
      // Caveat: If we can get the `MemberExprRefResolution` from the `resolved_member_expr_refs`,
      // it means this member expr definitely contains module namespace ref.
      if let Some(resolved_ref) = member_expr_resolution.resolved {
        let pre = ctx.may_partial_namespace;
        ctx.may_partial_namespace =
          member_expr_resolution.target_commonjs_exported_symbol.is_some();
        member_expr_resolution.depended_refs.iter().for_each(|sym_ref| {
          if let Module::Normal(module) = &ctx.modules[sym_ref.owner] {
            module.stmt_infos.declared_stmts_by_symbol(sym_ref).iter().copied().for_each(
              |stmt_info_id| {
                include_statement(ctx, module, stmt_info_id);
              },
            );
          }
        });
        include_symbol(ctx, resolved_ref, include_kind);
        ctx.may_partial_namespace = pre;
      } else {
        // If it points to nothing, the expression will be rewritten as `void 0` and there's nothing we need to include
      }
    } else {
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
      include_symbol(ctx, *original_ref, include_kind);
    }
  });
}
