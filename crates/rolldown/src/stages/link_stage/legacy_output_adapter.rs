use arcstr::ArcStr;
use oxc_index::IndexVec;
use rolldown_common::{
  EntryPoint, Module, ModuleIdx, ModuleTable, PreserveEntrySignatures, RetainedExportSymbols,
  RuntimeModuleBrief, SymbolRef, SymbolRefDb, UsedExternalSymbols, UsedSymbolRefsBuilder,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_error::Diagnostics;
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  type_alias::{IndexEcmaAst, IndexStmtInfos},
  types::linking_metadata::LinkingMetadata,
};

use super::{
  LinkStageOutput,
  lazy_json_export_initializers::LazyJsonExportInitializers,
  passes::{
    CjsNamespaceMerges, CjsRoutingFinal, DynamicExports, EntryExportRoots, EnumInliningPresence,
    ExternalStarExports, FinalizedModuleDependencies, GlobalConstants,
    IncludedCommonJsExportSymbols, InclusionResults, MemberExprResolutions, ModuleExecutionOrders,
    ModuleFormats, ModuleSideEffects, ModuleWrappers, NormalExportChains,
    ReferenceImportRecordPatches, ResolvedExports, RetainedEntries, ShimmedMissingExports,
    SortedModules, TlaFacts, TreeShakeModulePatches,
  },
};

pub(super) struct LegacyOutputAdapter<'a> {
  pub execution_orders: &'a ModuleExecutionOrders,
  pub tla_facts: &'a TlaFacts,
  pub module_formats: ModuleFormats,
  pub module_wrappers: ModuleWrappers,
  pub dynamic_exports: &'a DynamicExports,
  pub module_side_effects: &'a ModuleSideEffects,
  pub resolved_exports: ResolvedExports,
  pub included_commonjs_export_symbols: IncludedCommonJsExportSymbols,
  pub cjs_routing: CjsRoutingFinal,
  pub member_expr_resolutions: MemberExprResolutions,
  pub shimmed_missing_exports: ShimmedMissingExports,
  pub entry_export_roots: EntryExportRoots,
  pub external_star_exports: ExternalStarExports,
  pub inclusion: InclusionResults,
  pub finalized_dependencies: FinalizedModuleDependencies,
  pub retained_entries: RetainedEntries,
  pub sorted_modules: SortedModules,
  pub cjs_namespace_merges: CjsNamespaceMerges,
  pub external_namespace_merges: FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>>,
  pub global_constants: GlobalConstants,
  pub normal_export_chains: NormalExportChains,
  pub tree_shake_patches: TreeShakeModulePatches,
  pub reference_patches: ReferenceImportRecordPatches,
  pub used_symbol_refs: UsedSymbolRefsBuilder,
  pub used_external_symbols: UsedExternalSymbols,
  pub enum_inlining: EnumInliningPresence,
  pub lazy_json_export_initializers: LazyJsonExportInitializers,
}

fn apply_deferred_module_patches(
  module_table: &mut rolldown_common::ModuleTable,
  tree_shake_patches: TreeShakeModulePatches,
  reference_patches: ReferenceImportRecordPatches,
) {
  let (json_non_self_references, dead_dynamic_imports) = tree_shake_patches.into_parts();
  for (module_idx, symbols) in json_non_self_references {
    let module = module_table[module_idx]
      .as_normal_mut()
      .expect("JSON tree-shaking patches must target normal modules");
    module.ecma_view.json_module_none_self_reference_included_symbol = Some(Box::new(symbols));
  }
  for (module_idx, record_idx) in dead_dynamic_imports {
    let module = module_table[module_idx]
      .as_normal_mut()
      .expect("dead dynamic-import patches must target normal modules");
    module.import_records[record_idx]
      .meta
      .insert(rolldown_common::ImportRecordMeta::DeadDynamicImport);
  }
  reference_patches.apply(module_table);
}

fn project_cjs_routing(
  module_table: &rolldown_common::ModuleTable,
  metas: &mut IndexVec<ModuleIdx, LinkingMetadata>,
  cjs_routing: CjsRoutingFinal,
) {
  assert_eq!(cjs_routing.module_count(), module_table.modules.len());
  for (importer_idx, routes) in cjs_routing.into_importers() {
    assert!(
      module_table.modules.get(importer_idx).is_some_and(|module| module.as_normal().is_some()),
      "CJS routing importer {importer_idx:?} must be an in-range normal module"
    );
    for (symbol_ref, importee_idx) in &routes {
      assert_eq!(
        symbol_ref.owner, importer_idx,
        "CJS namespace route must be owned by its importer"
      );
      assert!(
        module_table.modules.get(*importee_idx).is_some_and(|module| module.as_normal().is_some()),
        "CJS namespace route target {importee_idx:?} must be an in-range normal module"
      );
    }
    metas[importer_idx].import_record_ns_to_cjs_module = routes;
  }
}

impl LegacyOutputAdapter<'_> {
  #[expect(
    clippy::too_many_arguments,
    reason = "the facade is already gone, so the boundary lists each final field explicitly"
  )]
  pub(super) fn finish(
    self,
    mut module_table: ModuleTable,
    symbols: SymbolRefDb,
    stmt_infos: IndexStmtInfos,
    runtime: RuntimeModuleBrief,
    diagnostics: Diagnostics,
    ast_table: IndexEcmaAst,
    dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
    overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
    entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
    user_defined_entry_modules: FxHashSet<ModuleIdx>,
  ) -> (LinkStageOutput, IndexEcmaAst, UsedSymbolRefsBuilder) {
    let module_count = module_table.modules.len();
    apply_deferred_module_patches(
      &mut module_table,
      self.tree_shake_patches,
      self.reference_patches,
    );

    for (module_idx, exec_order) in self.execution_orders.assigned() {
      match &mut module_table[module_idx] {
        Module::Normal(module) => {
          debug_assert_eq!(module.exec_order, u32::MAX);
          module.exec_order = exec_order;
        }
        Module::External(module) => {
          debug_assert_eq!(module.exec_order, u32::MAX);
          module.exec_order = exec_order;
        }
      }
    }
    assert_eq!(self.module_formats.module_count(), module_count);
    for (module_idx, format) in self.module_formats.normal_modules() {
      module_table[module_idx]
        .as_normal_mut()
        .expect("normal module format must target a normal module")
        .exports_kind = format;
    }
    assert_eq!(self.module_side_effects.module_count(), module_count);
    for module_idx in 0..module_count {
      let module_idx = ModuleIdx::from_usize(module_idx);
      if let Some(module) = module_table[module_idx].as_normal_mut() {
        module.side_effects = self.module_side_effects.get(module_idx);
      }
    }

    let mut metas = module_table
      .modules
      .iter()
      .map(|_| LinkingMetadata::default())
      .collect::<IndexVec<ModuleIdx, _>>();
    assert_eq!(self.tla_facts.module_count(), module_count);
    for module_idx in self.tla_facts.modules() {
      metas[module_idx].is_tla_or_contains_tla_dependency = true;
    }
    assert_eq!(self.dynamic_exports.module_count(), module_count);
    for module_idx in self.dynamic_exports.modules() {
      metas[module_idx].has_dynamic_exports = true;
    }
    assert_eq!(self.module_wrappers.module_count(), module_count);
    for (module_idx, declaration, required_by_other_module) in self.module_wrappers.modules() {
      let meta = &mut metas[module_idx];
      meta.required_by_other_module = required_by_other_module;
      match declaration {
        super::passes::WrapperDeclaration::None => {
          meta.set_wrap_kind(rolldown_common::WrapKind::None);
        }
        super::passes::WrapperDeclaration::Cjs { wrapper_ref, wrapper_stmt_info } => {
          meta.set_wrap_kind(rolldown_common::WrapKind::Cjs);
          meta.wrapper_ref = Some(wrapper_ref);
          meta.wrapper_stmt_info = Some(wrapper_stmt_info);
        }
        super::passes::WrapperDeclaration::Esm { wrapper_ref, wrapper_stmt_info } => {
          meta.set_wrap_kind(rolldown_common::WrapKind::Esm);
          meta.wrapper_ref = Some(wrapper_ref);
          meta.wrapper_stmt_info = Some(wrapper_stmt_info);
        }
      }
    }

    assert_eq!(self.resolved_exports.module_count(), module_count);
    for (module_idx, exports) in self.resolved_exports.into_slots().into_iter_enumerated() {
      match (&module_table[module_idx], exports) {
        (Module::Normal(_), Some(exports)) => {
          let (resolved, sorted) = exports.into_parts();
          metas[module_idx].resolved_exports = resolved;
          metas[module_idx].sorted_and_non_ambiguous_resolved_exports = sorted;
        }
        (Module::External(_), None) => {}
        (Module::Normal(_), None) => panic!("normal module {module_idx:?} has no export slot"),
        (Module::External(_), Some(_)) => {
          panic!("external module {module_idx:?} has an export slot")
        }
      }
    }
    assert_eq!(self.included_commonjs_export_symbols.module_count(), module_count);
    for (module_idx, symbols) in
      self.included_commonjs_export_symbols.into_slots().into_iter_enumerated()
    {
      match (&module_table[module_idx], symbols) {
        (Module::Normal(_), Some(symbols)) => {
          metas[module_idx].included_commonjs_export_symbol = symbols;
        }
        (Module::External(_), None) => {}
        (Module::Normal(_), None) => {
          panic!("normal module {module_idx:?} has no included-CommonJS-export slot")
        }
        (Module::External(_), Some(_)) => {
          panic!("external module {module_idx:?} has an included-CommonJS-export slot")
        }
      }
    }
    project_cjs_routing(&module_table, &mut metas, self.cjs_routing);
    assert_eq!(self.member_expr_resolutions.module_count(), module_count);
    for (module_idx, resolutions) in
      self.member_expr_resolutions.into_slots().into_iter_enumerated()
    {
      match (&module_table[module_idx], resolutions) {
        (Module::Normal(_), Some(resolutions)) => {
          metas[module_idx].resolved_member_expr_refs = resolutions;
        }
        (Module::External(_), None) => {}
        (Module::Normal(_), None) => {
          panic!("normal module {module_idx:?} has no member-resolution slot")
        }
        (Module::External(_), Some(_)) => {
          panic!("external module {module_idx:?} has a member-resolution slot")
        }
      }
    }
    assert_eq!(self.shimmed_missing_exports.module_count(), module_count);
    for (module_idx, shims) in self.shimmed_missing_exports.into_slots().into_iter_enumerated() {
      match (&module_table[module_idx], shims) {
        (Module::Normal(_), Some(shims)) => metas[module_idx].shimmed_missing_exports = shims,
        (Module::External(_), None) => {}
        (Module::Normal(_), None) => {
          panic!("normal module {module_idx:?} has no missing-export shim slot")
        }
        (Module::External(_), Some(_)) => {
          panic!("external module {module_idx:?} has a missing-export shim slot")
        }
      }
    }
    for (module_idx, roots) in self.entry_export_roots.into_entries() {
      metas[module_idx]
        .referenced_symbols_by_entry_point_chunk
        .extend(roots.into_iter().map(|root| (root.symbol_ref, root.came_from_commonjs)));
    }
    assert_eq!(self.external_star_exports.module_count(), module_count);
    for (module_idx, records) in self.external_star_exports.into_inner().into_iter_enumerated() {
      metas[module_idx].star_exports_from_external_modules = records;
    }

    let (stmt_included, module_included, namespace_reasons) = self.inclusion.into_parts();
    assert_eq!(stmt_included.len(), module_count);
    assert_eq!(namespace_reasons.len(), module_count);
    for (module_idx, stmt_included) in stmt_included.into_iter_enumerated() {
      metas[module_idx].stmt_info_included = stmt_included;
      metas[module_idx].is_included = module_included.has_bit(module_idx);
      metas[module_idx].module_namespace_included_reason = namespace_reasons[module_idx];
    }
    let (dependencies, load_dependencies, runtime_requirements, side_effectful_runtime) =
      self.finalized_dependencies.into_parts();
    assert_eq!(dependencies.len(), module_count);
    assert_eq!(load_dependencies.len(), module_count);
    assert_eq!(runtime_requirements.len(), module_count);
    for (((module_idx, dependencies), load_dependencies), runtime_requirements) in
      dependencies.into_iter_enumerated().zip(load_dependencies).zip(runtime_requirements)
    {
      metas[module_idx].dependencies = dependencies;
      metas[module_idx].load_dependencies = load_dependencies;
      metas[module_idx].depended_runtime_helper = runtime_requirements;
      metas[module_idx].has_side_effectful_runtime_dep = side_effectful_runtime.has_bit(module_idx);
    }

    tracing::trace!(modules = module_count, "assembled legacy link output");
    let output = LinkStageOutput {
      module_table,
      entries: self.retained_entries.into_inner(),
      sorted_modules: self.sorted_modules.into_inner(),
      metas,
      symbol_db: symbols,
      stmt_infos,
      runtime,
      diagnostics,
      used_external_symbols: self.used_external_symbols,
      retained_export_symbols: RetainedExportSymbols::default(),
      dynamic_import_exports_usage_map,
      safely_merge_cjs_ns_map: self.cjs_namespace_merges.into_legacy(),
      external_import_namespace_merger: self.external_namespace_merges,
      overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids,
      global_constant_symbol_map: self.global_constants.into_legacy(),
      normal_symbol_exports_chain_map: self.normal_export_chains.into_inner(),
      lazy_json_export_initializers: self.lazy_json_export_initializers,
      user_defined_entry_modules,
      has_enum_inlining: self.enum_inlining.get(),
    };
    #[cfg(feature = "testing")]
    let output = {
      let mut output = output;
      super::testing::observe_link_output(&mut output);
      output
    };
    (output, ast_table, self.used_symbol_refs)
  }
}

#[cfg(test)]
mod tests {
  use oxc::{semantic::SymbolId, span::Span};
  use oxc_index::IndexVec;
  use rolldown_common::{ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleIdx, SymbolRef};
  use rustc_hash::{FxHashMap, FxHashSet};

  use super::{apply_deferred_module_patches, project_cjs_routing};
  use crate::{
    stages::link_stage::passes::{
      TreeShakeModulePatches,
      test_utils::{
        cjs_routing_final, external_module, module_idx, module_table, normal_module,
        reference_import_record_patches,
      },
    },
    types::linking_metadata::LinkingMetadata,
  };

  fn symbol(owner: usize, symbol: usize) -> SymbolRef {
    SymbolRef { owner: module_idx(owner), symbol: SymbolId::new(symbol) }
  }

  #[test]
  fn deferred_patches_replace_json_symbols_and_preserve_both_import_record_flags() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(0, 1))]),
      normal_module(1, false, Vec::new()),
    ]);
    let old_symbol = symbol(0, 1);
    let replacement = symbol(0, 2);
    modules[module_idx(0)]
      .as_normal_mut()
      .expect("normal module")
      .ecma_view
      .json_module_none_self_reference_included_symbol =
      Some(Box::new(FxHashSet::from_iter([old_symbol])));
    let tree_shake_patches = TreeShakeModulePatches::new(
      FxHashMap::from_iter([(module_idx(0), FxHashSet::from_iter([replacement]))]),
      vec![(module_idx(0), ImportRecordIdx::from_usize(0))],
    );
    let reference_patches =
      reference_import_record_patches(2, [(module_idx(0), ImportRecordIdx::from_usize(0))]);

    apply_deferred_module_patches(&mut modules, tree_shake_patches, reference_patches);

    let module = modules[module_idx(0)].as_normal().expect("normal module");
    assert_eq!(
      module
        .ecma_view
        .json_module_none_self_reference_included_symbol
        .as_deref()
        .expect("JSON replacement"),
      &FxHashSet::from_iter([replacement])
    );
    let meta = module.import_records[ImportRecordIdx::from_usize(0)].meta;
    assert!(meta.contains(ImportRecordMeta::DeadDynamicImport));
    assert!(meta.contains(ImportRecordMeta::CallRuntimeRequire));
  }

  #[test]
  fn cjs_routing_rejects_non_normal_importers_foreign_symbols_and_non_normal_targets() {
    let modules = module_table(vec![
      normal_module(0, false, Vec::new()),
      normal_module(1, false, Vec::new()),
      external_module(2, "external"),
    ]);
    let make_metas =
      || (0..3).map(|_| LinkingMetadata::default()).collect::<IndexVec<ModuleIdx, _>>();
    let rejects = [
      cjs_routing_final(3, [(module_idx(2), symbol(2, 0), module_idx(1))]),
      cjs_routing_final(3, [(module_idx(0), symbol(1, 0), module_idx(1))]),
      cjs_routing_final(3, [(module_idx(0), symbol(0, 0), module_idx(2))]),
      cjs_routing_final(3, [(module_idx(0), symbol(0, 0), module_idx(3))]),
    ];

    for routing in rejects {
      let mut metas = make_metas();
      assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
          project_cjs_routing(&modules, &mut metas, routing);
        }))
        .is_err()
      );
    }
  }
}
