use std::cmp::Reverse;

#[cfg(target_family = "wasm")]
use itertools::Itertools as _;
use oxc_index::IndexVec;
use petgraph::prelude::DiGraphMap;
use rolldown_common::{
  ConstExportMeta, EntryPoint, EntryPointKind, ExportsKind, ImportKind, ImportRecordIdx,
  ImportRecordMeta, MemberExprRef, MemberExprRefResolution, Module, ModuleIdx,
  ModuleNamespaceIncludedReason, RuntimeHelper, SymbolRef, UsedExternalSymbols,
  UsedSymbolRefsBuilder, dynamic_import_usage::DynamicImportExportsUsage,
  side_effects::DeterminedSideEffects,
};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  IndexBitSet,
  index_vec_ext::IndexVecRefExt,
  indexmap::FxIndexMap,
  rayon::{IntoParallelRefIterator, ParallelIterator},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::stages::link_stage::{
  passes::{
    CjsRoutingFinal, EntryPlanDraft, GlobalConstants, IncludedCommonJsExportSymbols,
    InclusionResults, MemberExprResolutions, ModuleDependenciesDraft, ModuleFormats,
    ModuleRuntimeRequirementsDraft, ModuleSideEffects, ModuleWrappers, NormalExportChains,
    ResolvedExports, RetainedEntries, TreeShakeInput, TreeShakeModulePatches, TreeShakeOutput,
  },
  tree_shaking::inclusion_core::{
    InclusionConfig, InclusionCoreContext, InclusionFacts, InclusionModuleFacts, TreeShakingConfig,
    WorkItem, compute_body_demand_keys_core, include_cjs_bailout_exports,
    include_declaring_statements, include_module, include_runtime_symbol,
    include_symbol_and_check_cjs_bailout, preserve_reexported_interfaces,
  },
};

struct TreeShakeFacts<'a> {
  module_formats: &'a ModuleFormats,
  module_side_effects: &'a ModuleSideEffects,
  cjs_routing: &'a CjsRoutingFinal,
  resolved_exports: &'a ResolvedExports,
  included_commonjs_export_symbols: &'a IncludedCommonJsExportSymbols,
  dependencies: &'a ModuleDependenciesDraft,
  member_expr_resolutions: &'a MemberExprResolutions,
  module_wrappers: &'a ModuleWrappers,
  global_constants: &'a GlobalConstants,
  normal_export_chains: &'a NormalExportChains,
}

impl InclusionModuleFacts for TreeShakeFacts<'_> {
  fn exports_kind(&self, module_idx: ModuleIdx) -> ExportsKind {
    self.module_formats.get(module_idx).expect("tree-shaking facts require a normal module")
  }

  fn side_effects(&self, module_idx: ModuleIdx) -> DeterminedSideEffects {
    self.module_side_effects.get(module_idx)
  }
}

impl InclusionFacts for TreeShakeFacts<'_> {
  fn cjs_namespace_target(
    &self,
    importer_idx: ModuleIdx,
    namespace_ref: SymbolRef,
  ) -> Option<ModuleIdx> {
    self.cjs_routing.namespace_target(importer_idx, namespace_ref)
  }

  fn resolved_export_symbol(&self, module_idx: ModuleIdx, name: &str) -> Option<SymbolRef> {
    self.resolved_exports.get(module_idx, name).map(|export| export.symbol_ref)
  }

  fn commonjs_export_symbols(&self, module_idx: ModuleIdx) -> impl Iterator<Item = SymbolRef> + '_ {
    self
      .resolved_exports
      .iter(module_idx)
      .filter(|(_, export)| export.came_from_commonjs)
      .map(|(_, export)| export.symbol_ref)
  }

  fn included_commonjs_export_symbols(
    &self,
    module_idx: ModuleIdx,
  ) -> impl Iterator<Item = SymbolRef> + '_ {
    self.included_commonjs_export_symbols.iter(module_idx)
  }

  fn dependencies(&self, module_idx: ModuleIdx) -> impl Iterator<Item = ModuleIdx> + '_ {
    self.dependencies.iter(module_idx)
  }

  fn member_expr_resolution<'a>(
    &'a self,
    module_idx: ModuleIdx,
    member_expr_ref: &MemberExprRef,
  ) -> Option<&'a MemberExprRefResolution> {
    self
      .member_expr_resolutions
      .get(module_idx)
      .and_then(|resolutions| member_expr_ref.resolution(resolutions))
  }

  fn esm_wrapper_ref(&self, module_idx: ModuleIdx) -> Option<SymbolRef> {
    self.module_wrappers.esm_wrapper_ref(module_idx)
  }

  fn constant_export(&self, symbol_ref: &SymbolRef) -> Option<&ConstExportMeta> {
    self.global_constants.get(symbol_ref)
  }

  fn normal_export_chain(&self, symbol_ref: &SymbolRef) -> &[SymbolRef] {
    self.normal_export_chains.get(symbol_ref).unwrap_or(&[])
  }

  fn normal_export_chains(&self) -> impl Iterator<Item = (SymbolRef, &[SymbolRef])> + '_ {
    self.normal_export_chains.iter()
  }
}

fn assert_tree_shake_layout(input: TreeShakeInput<'_>) {
  let module_count = input.module_table.modules.len();
  for (domain, actual) in [
    ("statement", input.stmt_infos.len()),
    ("symbol", input.symbols.inner().len()),
    ("format", input.module_formats.module_count()),
    ("wrapper", input.module_wrappers.module_count()),
    ("resolved-export", input.resolved_exports.module_count()),
    ("CJS-routing", input.cjs_routing.module_count()),
    ("dependency", input.dependencies.module_count()),
    ("member-resolution", input.member_expr_resolutions.module_count()),
    ("side-effect", input.module_side_effects.module_count()),
    ("included-CommonJS-export", input.included_commonjs_export_symbols.module_count()),
    ("statement-runtime-requirement", input.statement_runtime_requirements.slots().len()),
  ] {
    assert_eq!(actual, module_count, "{domain} layout must match modules before tree shaking");
  }
  assert!(
    input
      .module_table
      .modules
      .get(input.runtime.id())
      .is_some_and(|module| module.as_normal().is_some()),
    "runtime must be an in-range normal module before tree shaking"
  );
  for (module_idx, module) in input.module_table.modules.iter_enumerated() {
    let valid = match module {
      Module::Normal(module) => {
        module.idx == module_idx
          && input.module_formats.get(module_idx).is_some()
          && input.resolved_exports.has_normal_slot(module_idx)
          && input.member_expr_resolutions.has_normal_slot(module_idx)
          && input.symbols.inner()[module_idx].is_some()
      }
      Module::External(module) => {
        module.idx == module_idx
          && input.module_formats.get(module_idx).is_none()
          && !input.resolved_exports.has_normal_slot(module_idx)
          && !input.member_expr_resolutions.has_normal_slot(module_idx)
          && input.symbols.inner()[module_idx].is_some()
      }
    };
    assert!(valid, "tree-shaking layout is malformed at module {module_idx:?}");
  }
}

fn construct_dynamic_entry_graph(
  modules: &rolldown_common::IndexModules,
  graph: &mut DiGraphMap<ModuleIdx, ()>,
  visited: &mut FxHashSet<ModuleIdx>,
  root_node: &mut ModuleIdx,
  current_node: ModuleIdx,
) -> Option<()> {
  if !visited.insert(current_node) {
    return Some(());
  }
  let module = modules[current_node].as_normal()?;
  for record in &module.import_records {
    let Some(module_idx) = record.resolved_module else {
      continue;
    };
    if record.kind == ImportKind::DynamicImport {
      let seen = graph.contains_node(module_idx);
      if *root_node != module_idx {
        graph.add_edge(*root_node, module_idx, ());
        if seen {
          continue;
        }
      }
      let previous = *root_node;
      *root_node = module_idx;
      construct_dynamic_entry_graph(modules, graph, visited, root_node, module_idx);
      *root_node = previous;
      continue;
    }
    construct_dynamic_entry_graph(modules, graph, visited, root_node, module_idx);
  }
  Some(())
}

fn sort_dynamic_entries_by_topological_order(
  modules: &rolldown_common::IndexModules,
  dynamic_entries: &mut [EntryPoint],
) -> FxHashSet<ModuleIdx> {
  let mut graph = DiGraphMap::new();
  for entry in dynamic_entries.iter() {
    let mut root = entry.idx;
    let mut visited = FxHashSet::default();
    construct_dynamic_entry_graph(modules, &mut graph, &mut visited, &mut root, entry.idx);
  }
  let mut cycled = FxHashSet::default();
  let order = petgraph::algo::tarjan_scc(&graph)
    .into_iter()
    .enumerate()
    .filter(|(_, component)| {
      if component.len() > 1 {
        cycled.extend(component.iter().copied());
        false
      } else {
        true
      }
    })
    .map(|(order, component)| (component[0], order))
    .collect::<FxHashMap<_, _>>();
  dynamic_entries.sort_by_key(|entry| {
    order.get(&entry.idx).map_or(Reverse(usize::MAX), |order| Reverse(*order))
  });
  cycled
}

fn dynamic_entry_dead_records(
  entry: &EntryPoint,
  input: TreeShakeInput<'_>,
  stmt_included: &super::tree_shaking::StmtInclusionVec,
) -> Option<Vec<(ModuleIdx, ImportRecordIdx)>> {
  let mut dead_records = Vec::new();
  let alive = match entry.kind {
    EntryPointKind::UserDefined | EntryPointKind::EmittedUserDefined => true,
    EntryPointKind::DynamicImport => {
      let exports_unused = input.dynamic_import_usage.get(&entry.idx).is_some_and(
        |usage| matches!(usage, DynamicImportExportsUsage::Partial(exports) if exports.is_empty()),
      );
      entry.related_stmt_infos.iter().any(|(module_idx, stmt_idx, node_id, record_idx)| {
        if input.unreachable_dynamic_imports.contains(*module_idx, *node_id) {
          return false;
        }
        let module = input.module_table[*module_idx]
          .as_normal()
          .expect("dynamic-entry relation must be owned by a normal module");
        let record = &module.import_records[*record_idx];
        let pure_and_side_effect_free =
          record.meta.contains(ImportRecordMeta::TopLevelPureDynamicImport)
            && !input.module_side_effects.get(record.into_resolved_module()).has_side_effects();
        let alive = stmt_included[*module_idx].has_bit(*stmt_idx)
          && (!exports_unused || !pure_and_side_effect_free);
        if !alive && pure_and_side_effect_free {
          dead_records.push((*module_idx, *record_idx));
        }
        alive
      })
    }
  };
  (!alive).then_some(dead_records)
}

fn process_dynamic_entry(
  entry: &EntryPoint,
  cycled: &FxHashSet<ModuleIdx>,
  input: TreeShakeInput<'_>,
  context: &mut InclusionCoreContext<'_, TreeShakeFacts<'_>>,
  dead_records: &mut Vec<(ModuleIdx, ImportRecordIdx)>,
) -> bool {
  if !cycled.contains(&entry.idx)
    && let Some(records) = dynamic_entry_dead_records(entry, input, context.is_included_vec)
  {
    dead_records.extend(records);
    return false;
  }
  let Some(module) = input.module_table[entry.idx].as_normal() else {
    return true;
  };
  for root in input.entry_export_roots.get(entry.idx).unwrap_or_default() {
    if context.modules[root.symbol_ref.owner].as_normal().is_some() {
      include_declaring_statements(context, &root.symbol_ref);
      include_symbol_and_check_cjs_bailout(
        context,
        root.symbol_ref,
        super::tree_shaking::SymbolIncludeReason::EntryExport,
      );
    }
  }
  include_module(context, module);
  true
}

fn normalize_runtime_requirements(
  module_table: &rolldown_common::ModuleTable,
  statement_runtime_requirements: &super::passes::StatementRuntimeRequirements,
  stmt_included: &super::tree_shaking::StmtInclusionVec,
  module_included: &super::tree_shaking::ModuleInclusionVec,
) -> ModuleRuntimeRequirementsDraft {
  let requirements = module_table
    .modules
    .par_iter()
    .zip_eq(statement_runtime_requirements.slots().par_iter())
    .map(|(module, requirements)| {
      let Some(module) = module.as_normal() else {
        return RuntimeHelper::default();
      };
      let mut normalized = RuntimeHelper::default();
      for (helper, stmt_info_idxs) in requirements.iter() {
        if stmt_info_idxs.is_empty() {
          continue;
        }
        let any_included =
          stmt_info_idxs.iter().any(|stmt_idx| stmt_included[module.idx].has_bit(*stmt_idx));
        normalized.set(
          helper,
          any_included
            || (module.id != rolldown_common::RUNTIME_MODULE_ID
              && !module_included.has_bit(module.idx)),
        );
      }
      normalized
    })
    .collect::<Vec<_>>();
  ModuleRuntimeRequirementsDraft::new(IndexVec::from_vec(requirements))
}

fn collect_depended_runtime_helpers(
  input: TreeShakeInput<'_>,
  module_included: &super::tree_shaking::ModuleInclusionVec,
  requirements: &ModuleRuntimeRequirementsDraft,
) -> RuntimeHelper {
  let iter = input.module_table.modules.par_iter_enumerated().filter_map(|(module_idx, module)| {
    module
      .as_normal()
      .filter(|_| module_included.has_bit(module_idx))
      .map(|_| requirements.get(module_idx))
  });
  #[cfg(not(target_family = "wasm"))]
  let result = iter.reduce(RuntimeHelper::default, |left, right| left | right);
  #[cfg(target_family = "wasm")]
  let result = iter.reduce(|left, right| left | right).unwrap_or_default();
  result
}

pub(in crate::stages::link_stage) fn run_tree_shake(
  input: TreeShakeInput<'_>,
  entry_plan: EntryPlanDraft,
) -> TreeShakeOutput {
  assert_tree_shake_layout(input);
  let module_count = input.module_table.modules.len();
  let mut stmt_included = input
    .module_table
    .modules
    .iter()
    .zip(input.stmt_infos.iter())
    .map(|(module, stmt_infos)| {
      module.as_normal().map_or(IndexBitSet::default(), |_| IndexBitSet::new(stmt_infos.len()))
    })
    .collect::<IndexVec<ModuleIdx, _>>();
  let mut module_included = IndexBitSet::new(module_count);
  let mut phase_one_namespace_reasons =
    oxc_index::index_vec![ModuleNamespaceIncludedReason::empty(); module_count];
  let mut used_symbol_refs = UsedSymbolRefsBuilder::default();
  let mut used_external_symbols = UsedExternalSymbols::default();
  let mut entries = entry_plan.into_entries();
  let entry_module_idxs = entries
    .values()
    .flatten()
    .filter(|entry| entry.kind.is_user_defined())
    .map(|entry| entry.idx)
    .collect::<FxHashSet<_>>();
  let facts = TreeShakeFacts {
    module_formats: input.module_formats,
    module_side_effects: input.module_side_effects,
    cjs_routing: input.cjs_routing,
    resolved_exports: input.resolved_exports,
    included_commonjs_export_symbols: input.included_commonjs_export_symbols,
    dependencies: input.dependencies,
    member_expr_resolutions: input.member_expr_resolutions,
    module_wrappers: input.module_wrappers,
    global_constants: input.global_constants,
    normal_export_chains: input.normal_export_chains,
  };
  let body_demand_keys = compute_body_demand_keys_core(
    &facts,
    &input.module_table.modules,
    input.stmt_infos,
    input.symbols,
    input.options.inclusion.tree_shaking_enabled,
    &entry_module_idxs,
  );
  let config = InclusionConfig {
    tree_shaking: TreeShakingConfig {
      enabled: input.options.inclusion.tree_shaking_enabled,
      commonjs: input.options.inclusion.commonjs_tree_shaking,
      property_write_side_effects: input.options.inclusion.property_write_side_effects,
    },
    inline_const_smart: input.options.inclusion.inline_const_smart,
    preserve_modules: input.options.preserve_modules,
    dev_mode: input.options.dev_mode,
  };

  let (user_entries, mut dynamic_entries): (Vec<_>, Vec<_>) = std::mem::take(&mut entries)
    .into_values()
    .flatten()
    .partition(|entry| entry.kind.is_user_defined());
  let mut bailout_modules = FxHashSet::default();
  let mut inclusion_changed = false;
  let mut json_non_self_references = FxHashMap::default();
  let mut body_demand_swept = FxHashSet::default();
  let mut pending = Vec::<WorkItem>::new();
  let mut dead_dynamic_imports = Vec::new();
  let mut included_dynamic_entries = FxHashSet::default();
  {
    let mut context = InclusionCoreContext {
      facts: &facts,
      modules: &input.module_table.modules,
      stmt_infos: input.stmt_infos,
      symbols: input.symbols,
      is_included_vec: &mut stmt_included,
      is_module_included_vec: &mut module_included,
      config,
      runtime_idx: input.runtime.id(),
      used_symbol_refs: &mut used_symbol_refs,
      used_external_symbols: &mut used_external_symbols,
      bailout_cjs_tree_shaking_modules: &mut bailout_modules,
      module_inclusion_changed: &mut inclusion_changed,
      module_namespace_included_reason: &mut phase_one_namespace_reasons,
      json_module_none_self_reference_included_symbol: &mut json_non_self_references,
      entry_module_idxs: &entry_module_idxs,
      body_demand_keys: &body_demand_keys,
      body_demand_swept: &mut body_demand_swept,
      pending: &mut pending,
    };
    for entry in &user_entries {
      let Some(module) = input.module_table[entry.idx].as_normal() else {
        continue;
      };
      context.bailout_cjs_tree_shaking_modules.insert(module.idx);
      for root in input.entry_export_roots.get(entry.idx).unwrap_or_default() {
        if context.modules[root.symbol_ref.owner].as_normal().is_some() {
          include_declaring_statements(&mut context, &root.symbol_ref);
          include_symbol_and_check_cjs_bailout(
            &mut context,
            root.symbol_ref,
            super::tree_shaking::SymbolIncludeReason::EntryExport,
          );
        }
      }
      include_module(&mut context, module);
    }

    let cycled =
      sort_dynamic_entries_by_topological_order(&input.module_table.modules, &mut dynamic_entries);
    loop {
      *context.module_inclusion_changed = false;
      let bailout = std::mem::take(context.bailout_cjs_tree_shaking_modules);
      include_cjs_bailout_exports(&mut context, bailout);
      for entry in &dynamic_entries {
        if included_dynamic_entries.contains(&entry.idx) {
          continue;
        }
        if process_dynamic_entry(entry, &cycled, input, &mut context, &mut dead_dynamic_imports) {
          included_dynamic_entries.insert(entry.idx);
        }
      }
      if !*context.module_inclusion_changed {
        break;
      }
    }
    preserve_reexported_interfaces(&mut context);
  }
  dynamic_entries.retain(|entry| included_dynamic_entries.contains(&entry.idx));

  let retained_entries = RetainedEntries::new(
    user_entries
      .into_iter()
      .chain(if input.options.code_splitting_disabled {
        itertools::Either::Left(std::iter::empty())
      } else {
        itertools::Either::Right(dynamic_entries.into_iter())
      })
      .fold(FxIndexMap::default(), |mut entries, entry| {
        entries.entry(entry.idx).or_default().push(entry);
        entries
      }),
  );
  let runtime_requirements = normalize_runtime_requirements(
    input.module_table,
    input.statement_runtime_requirements,
    &stmt_included,
    &module_included,
  );
  let depended_runtime_helpers =
    collect_depended_runtime_helpers(input, &module_included, &runtime_requirements);

  let mut phase_two_bailout = FxHashSet::default();
  let mut phase_two_changed = false;
  let mut phase_two_namespace_reasons =
    oxc_index::index_vec![ModuleNamespaceIncludedReason::empty(); module_count];
  let mut phase_two_json = FxHashMap::default();
  let mut phase_two_body_demand_swept = FxHashSet::default();
  let mut phase_two_pending = Vec::new();
  {
    let mut runtime_context = InclusionCoreContext {
      facts: &facts,
      modules: &input.module_table.modules,
      stmt_infos: input.stmt_infos,
      symbols: input.symbols,
      is_included_vec: &mut stmt_included,
      is_module_included_vec: &mut module_included,
      config,
      runtime_idx: input.runtime.id(),
      used_symbol_refs: &mut used_symbol_refs,
      used_external_symbols: &mut used_external_symbols,
      bailout_cjs_tree_shaking_modules: &mut phase_two_bailout,
      module_inclusion_changed: &mut phase_two_changed,
      module_namespace_included_reason: &mut phase_two_namespace_reasons,
      json_module_none_self_reference_included_symbol: &mut phase_two_json,
      entry_module_idxs: &entry_module_idxs,
      body_demand_keys: &body_demand_keys,
      body_demand_swept: &mut phase_two_body_demand_swept,
      pending: &mut phase_two_pending,
    };
    include_runtime_symbol(&mut runtime_context, input.runtime, depended_runtime_helpers);
  }

  let enum_inlining =
    super::passes::EnumInliningPresence::new(input.module_table.modules.iter().any(|module| {
      module.as_normal().is_some_and(|module| !module.ecma_view.enum_member_value_map.is_empty())
    }));
  TreeShakeOutput {
    retained_entries,
    inclusion: InclusionResults::new(stmt_included, module_included, phase_one_namespace_reasons),
    runtime_requirements,
    used_symbol_refs,
    used_external_symbols,
    module_patches: TreeShakeModulePatches::new(json_non_self_references, dead_dynamic_imports),
    enum_inlining,
  }
}

#[cfg(test)]
mod tests {
  use oxc::span::Span;
  use oxc_index::IndexVec;
  use rolldown_common::{
    EntryPointKind, ImportKind, RUNTIME_MODULE_ID, RuntimeHelper, StmtInfoIdx,
  };
  use rolldown_utils::IndexBitSet;

  use super::{normalize_runtime_requirements, sort_dynamic_entries_by_topological_order};
  use crate::stages::link_stage::{
    passes::test_utils::{
      entry_point, module_idx, module_table, normal_module, normal_module_with_id,
      statement_runtime_requirements,
    },
    tree_shaking::{ModuleInclusionVec, StmtInclusionVec},
  };

  #[test]
  fn normalizes_runtime_requirements_from_phase_one_inclusion_only() {
    let modules = module_table(vec![
      normal_module_with_id(0, &RUNTIME_MODULE_ID, false, Vec::new()),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
      normal_module(3, false, Vec::new()),
    ]);
    let requirements = statement_runtime_requirements(
      4,
      [
        (module_idx(0), RuntimeHelper::Require, StmtInfoIdx::from_usize(0)),
        (module_idx(1), RuntimeHelper::Name, StmtInfoIdx::from_usize(0)),
        (module_idx(2), RuntimeHelper::ToEsm, StmtInfoIdx::from_usize(0)),
        (module_idx(3), RuntimeHelper::CopyProps, StmtInfoIdx::from_usize(0)),
      ],
    );
    let mut statement_inclusion =
      (0..4).map(|_| IndexBitSet::new(1)).collect::<IndexVec<rolldown_common::ModuleIdx, _>>();
    statement_inclusion[module_idx(1)].set_bit(StmtInfoIdx::from_usize(0));
    let statement_inclusion: StmtInclusionVec = statement_inclusion;
    let mut module_inclusion: ModuleInclusionVec = IndexBitSet::new(4);
    module_inclusion.set_bit(module_idx(1));
    module_inclusion.set_bit(module_idx(3));

    let normalized = normalize_runtime_requirements(
      &modules,
      &requirements,
      &statement_inclusion,
      &module_inclusion,
    );

    assert!(normalized.get(module_idx(1)).contains(RuntimeHelper::Name));
    assert!(normalized.get(module_idx(2)).contains(RuntimeHelper::ToEsm));
    assert!(!normalized.get(module_idx(0)).contains(RuntimeHelper::Require));
    assert!(!normalized.get(module_idx(3)).contains(RuntimeHelper::CopyProps));
  }

  #[test]
  fn orders_dynamic_entry_ancestors_before_descendants_and_marks_cycles() {
    let chain = module_table(vec![
      normal_module(0, false, Vec::new()),
      normal_module(1, false, vec![(ImportKind::DynamicImport, Some(2), Span::new(0, 1))]),
      normal_module(2, false, vec![(ImportKind::DynamicImport, Some(3), Span::new(1, 2))]),
      normal_module(3, false, Vec::new()),
    ]);
    let mut entries = [3, 1, 2].map(|index| entry_point(index, EntryPointKind::DynamicImport));
    let cycled = sort_dynamic_entries_by_topological_order(&chain.modules, &mut entries);
    assert!(cycled.is_empty());
    assert_eq!(entries.map(|entry| entry.idx), [1, 2, 3].map(module_idx));

    let cycle = module_table(vec![
      normal_module(0, false, Vec::new()),
      normal_module(1, false, vec![(ImportKind::DynamicImport, Some(2), Span::new(0, 1))]),
      normal_module(2, false, vec![(ImportKind::DynamicImport, Some(1), Span::new(1, 2))]),
    ]);
    let mut entries = [1, 2].map(|index| entry_point(index, EntryPointKind::DynamicImport));
    let cycled = sort_dynamic_entries_by_topological_order(&cycle.modules, &mut entries);
    assert_eq!(cycled, [module_idx(1), module_idx(2)].into_iter().collect());
  }
}
