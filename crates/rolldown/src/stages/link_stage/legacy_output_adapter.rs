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
    clippy::too_many_lines,
    reason = "the one-shot boundary lists every final field and keeps dense validation plus physical assembly visible in one place"
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
    let LegacyOutputAdapter {
      execution_orders,
      tla_facts,
      module_formats,
      module_wrappers,
      dynamic_exports,
      module_side_effects,
      resolved_exports,
      included_commonjs_export_symbols,
      cjs_routing,
      member_expr_resolutions,
      shimmed_missing_exports,
      entry_export_roots,
      external_star_exports,
      inclusion,
      finalized_dependencies,
      retained_entries,
      sorted_modules,
      cjs_namespace_merges,
      external_namespace_merges,
      global_constants,
      normal_export_chains,
      tree_shake_patches,
      reference_patches,
      used_symbol_refs,
      used_external_symbols,
      enum_inlining,
      lazy_json_export_initializers,
    } = self;
    let module_count = module_table.modules.len();

    let resolved_export_slots = resolved_exports.into_slots();
    let included_commonjs_export_symbol_slots = included_commonjs_export_symbols.into_slots();
    let member_expr_resolution_slots = member_expr_resolutions.into_slots();
    let shimmed_missing_export_slots = shimmed_missing_exports.into_slots();
    let external_star_export_slots = external_star_exports.into_inner();
    let (stmt_included_slots, module_included, namespace_reason_slots) = inclusion.into_parts();
    let (
      dependency_slots,
      load_dependency_slots,
      runtime_requirement_slots,
      side_effectful_runtime_dependencies,
    ) = finalized_dependencies.into_parts();

    for (domain, actual) in [
      ("AST", ast_table.len()),
      ("statement", stmt_infos.len()),
      ("symbol", symbols.inner().len()),
      ("execution-order", execution_orders.module_count()),
      ("TLA", tla_facts.module_count()),
      ("format", module_formats.module_count()),
      ("wrapper", module_wrappers.module_count()),
      ("dynamic-export", dynamic_exports.module_count()),
      ("side-effect", module_side_effects.module_count()),
      ("resolved-export", resolved_export_slots.len()),
      ("included-CommonJS-export", included_commonjs_export_symbol_slots.len()),
      ("CJS-routing", cjs_routing.module_count()),
      ("member-resolution", member_expr_resolution_slots.len()),
      ("missing-export shim", shimmed_missing_export_slots.len()),
      ("external-star export", external_star_export_slots.len()),
      ("statement-inclusion", stmt_included_slots.len()),
      ("namespace-reason", namespace_reason_slots.len()),
      ("dependency", dependency_slots.len()),
      ("load-dependency", load_dependency_slots.len()),
      ("runtime-requirement", runtime_requirement_slots.len()),
    ] {
      assert_eq!(actual, module_count, "{domain} layout must match modules at the Link boundary");
    }

    for (module_idx, module) in module_table.modules.iter_enumerated() {
      let valid = match module {
        Module::Normal(module) => {
          module.idx == module_idx
            && ast_table[module_idx].is_some()
            && symbols.inner()[module_idx].is_some()
            && module_formats.get(module_idx).is_some()
            && resolved_export_slots[module_idx].is_some()
            && included_commonjs_export_symbol_slots[module_idx].is_some()
            && member_expr_resolution_slots[module_idx].is_some()
            && shimmed_missing_export_slots[module_idx].is_some()
        }
        Module::External(module) => {
          module.idx == module_idx
            && ast_table[module_idx].is_none()
            && symbols.inner()[module_idx].is_some()
            && module_formats.get(module_idx).is_none()
            && std::matches!(
              module_wrappers.declaration(module_idx),
              super::passes::WrapperDeclaration::None
            )
            && resolved_export_slots[module_idx].is_none()
            && included_commonjs_export_symbol_slots[module_idx].is_none()
            && member_expr_resolution_slots[module_idx].is_none()
            && shimmed_missing_export_slots[module_idx].is_none()
            && external_star_export_slots[module_idx].is_empty()
        }
      };
      assert!(valid, "legacy output slot shape must match module {module_idx:?}");
    }

    apply_deferred_module_patches(&mut module_table, tree_shake_patches, reference_patches);

    let mut metas = itertools::izip!(
      module_table.modules.iter_mut_enumerated(),
      resolved_export_slots,
      included_commonjs_export_symbol_slots,
      member_expr_resolution_slots,
      shimmed_missing_export_slots,
      external_star_export_slots,
      stmt_included_slots,
      namespace_reason_slots,
      dependency_slots,
      load_dependency_slots,
      runtime_requirement_slots,
    )
    .map(
      |(
        (module_idx, module),
        resolved_exports,
        included_commonjs_export_symbols,
        member_expr_resolutions,
        shimmed_missing_exports,
        external_star_exports,
        stmt_included,
        namespace_reason,
        dependencies,
        load_dependencies,
        runtime_requirements,
      )| {
        let mut meta = LinkingMetadata::default();
        let exec_order = execution_orders.get(module_idx);

        match module {
          Module::Normal(module) => {
            if exec_order != u32::MAX {
              debug_assert_eq!(module.exec_order, u32::MAX);
              module.exec_order = exec_order;
            }
            let Some(format) = module_formats.get(module_idx) else {
              std::unreachable!("validated normal modules must have format slots");
            };
            module.exports_kind = format;
            module.side_effects = module_side_effects.get(module_idx);

            let Some(resolved_exports) = resolved_exports else {
              std::unreachable!("validated normal modules must have resolved-export slots");
            };
            let (resolved, sorted) = resolved_exports.into_parts();
            meta.resolved_exports = resolved;
            meta.sorted_and_non_ambiguous_resolved_exports = sorted;
            let Some(included_commonjs_export_symbols) = included_commonjs_export_symbols else {
              std::unreachable!(
                "validated normal modules must have included-CommonJS-export slots"
              );
            };
            meta.included_commonjs_export_symbol = included_commonjs_export_symbols;
            let Some(member_expr_resolutions) = member_expr_resolutions else {
              std::unreachable!("validated normal modules must have member-resolution slots");
            };
            meta.resolved_member_expr_refs = member_expr_resolutions;
            let Some(shimmed_missing_exports) = shimmed_missing_exports else {
              std::unreachable!("validated normal modules must have missing-export shim slots");
            };
            meta.shimmed_missing_exports = shimmed_missing_exports;
          }
          Module::External(module) => {
            if exec_order != u32::MAX {
              debug_assert_eq!(module.exec_order, u32::MAX);
              module.exec_order = exec_order;
            }
            debug_assert!(resolved_exports.is_none());
            debug_assert!(included_commonjs_export_symbols.is_none());
            debug_assert!(member_expr_resolutions.is_none());
            debug_assert!(shimmed_missing_exports.is_none());
          }
        }

        let (declaration, required_by_other_module) = module_wrappers.get(module_idx);
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
        meta.star_exports_from_external_modules = external_star_exports;
        meta.stmt_info_included = stmt_included;
        meta.is_included = module_included.has_bit(module_idx);
        meta.module_namespace_included_reason = namespace_reason;
        meta.dependencies = dependencies;
        meta.load_dependencies = load_dependencies;
        meta.depended_runtime_helper = runtime_requirements;
        meta.has_side_effectful_runtime_dep =
          side_effectful_runtime_dependencies.has_bit(module_idx);
        meta
      },
    )
    .collect::<IndexVec<ModuleIdx, LinkingMetadata>>();

    for module_idx in tla_facts.modules() {
      metas[module_idx].is_tla_or_contains_tla_dependency = true;
    }
    for module_idx in dynamic_exports.modules() {
      metas[module_idx].has_dynamic_exports = true;
    }
    project_cjs_routing(&module_table, &mut metas, cjs_routing);
    for (module_idx, roots) in entry_export_roots.into_entries() {
      metas[module_idx].referenced_symbols_by_entry_point_chunk = roots;
    }

    tracing::trace!(modules = module_count, "assembled legacy link output");
    let output = LinkStageOutput {
      module_table,
      entries: retained_entries.into_inner(),
      sorted_modules: sorted_modules.into_inner(),
      metas,
      symbol_db: symbols,
      stmt_infos,
      runtime,
      diagnostics,
      used_external_symbols,
      retained_export_symbols: RetainedExportSymbols::default(),
      dynamic_import_exports_usage_map,
      safely_merge_cjs_ns_map: cjs_namespace_merges.into_legacy(),
      external_import_namespace_merger: external_namespace_merges,
      overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids,
      global_constant_symbol_map: global_constants.into_legacy(),
      normal_symbol_exports_chain_map: normal_export_chains.into_inner(),
      lazy_json_export_initializers,
      user_defined_entry_modules,
      has_enum_inlining: enum_inlining.get(),
    };
    #[cfg(feature = "testing")]
    let output = {
      let mut output = output;
      super::testing::observe_link_output(&mut output);
      output
    };
    (output, ast_table, used_symbol_refs)
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
