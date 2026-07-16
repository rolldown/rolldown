use arcstr::ArcStr;
use rolldown_common::{
  ConstExportMeta, EntryPoint, ModuleIdx, ModuleTable, PreserveEntrySignatures,
  RetainedExportSymbols, RuntimeModuleBrief, SymbolRef, SymbolRefDb, UsedExternalSymbols,
  UsedSymbolRefsBuilder, dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_error::{Diagnostics, EventKindSwitcher};
use rolldown_utils::{
  indexmap::{FxIndexMap, FxIndexSet},
  pass::{PassPipelineCtx, run_infallible_pass},
};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions,
  type_alias::{IndexEcmaAst, IndexStmtInfos},
  types::linking_metadata::LinkingMetadataVec,
};

use super::scan_stage::NormalizedScanStageOutput;

mod cross_module_optimization_runner;
mod finalize_module_dependencies_runner;
mod generate_lazy_export;
pub mod lazy_json_export_initializers;
mod legacy_output_adapter;
mod non_splittable_json_defaults;
mod passes;
#[cfg(feature = "testing")]
pub mod testing;
mod tree_shake_runner;
mod tree_shaking;

pub use tree_shaking::{
  IncludeContext, ModuleInclusionVec, ModuleNamespaceReasonVec, StmtInclusionVec,
  SymbolIncludeReason, compute_body_demand_keys, include_runtime_symbol, include_symbol,
};

use lazy_json_export_initializers::LazyJsonExportInitializers;
use legacy_output_adapter::LegacyOutputAdapter;

use passes::{
  BindImportsInput, BindImportsOutput, BindImportsOwned, BindImportsPass, CanonicalizeEntriesPass,
  CollectEntryExportRootsInput, CollectEntryExportRootsPass, CollectExternalStarExportsPass,
  CollectInitialDependenciesPass, CollectResolvedExportsPass, ComputeCjsNamespaceMergesInput,
  ComputeCjsNamespaceMergesPass, ComputeCjsRoutingInput, ComputeCjsRoutingPass,
  ComputeDynamicExportsInput, ComputeDynamicExportsPass, ComputeModuleExecutionOrderInput,
  ComputeModuleExecutionOrderPass, ComputeTlaPass, ConstantExtractionInput,
  CreateSyntheticExportStatementsInput, CreateSyntheticExportStatementsPass,
  CreateWrapperDeclarationsInput, CreateWrapperDeclarationsOutput, CreateWrapperDeclarationsOwned,
  CreateWrapperDeclarationsPass, CrossModuleOptimizationInput, CrossModuleOptimizationOutput,
  CrossModuleOptimizationOwned, CrossModuleOptimizationPass, DetermineModuleFormatsInput,
  DetermineModuleFormatsPass, DetermineModuleSideEffectsInput, DetermineModuleSideEffectsPass,
  ExtractGlobalConstantsPass, FinalizeModuleDependenciesInput, FinalizeModuleDependenciesOwned,
  FinalizeModuleDependenciesPass, FinalizeResolvedExportsPass, NormalizeLazyExportsInput,
  NormalizeLazyExportsOutput, NormalizeLazyExportsOwned, NormalizeLazyExportsPass,
  PlanModuleWrappingInput, PlanModuleWrappingPass, ReferenceChunkingOptions,
  ReferenceNeededSymbolsInput, ReferenceNeededSymbolsOutput, ReferenceNeededSymbolsOwned,
  ReferenceNeededSymbolsPass, ReferenceTreeShakingOptions, ResolveMemberExpressionsInput,
  ResolveMemberExpressionsOutput, ResolveMemberExpressionsOwned, ResolveMemberExpressionsPass,
  TlaScanFacts, TreeShakeInclusionPolicy, TreeShakeInput, TreeShakeOptions, TreeShakeOutput,
  TreeShakeOwned, TreeShakePass,
};

/// Information about safely merged CJS namespaces for a module
#[derive(Debug, Default, Clone)]
pub struct SafelyMergeCjsNsInfo {
  /// Namespace symbol refs that can be merged into a single binding
  pub namespace_refs: Vec<SymbolRef>,
  /// Whether this CJS module needs `__toESM` interop (has namespace or default imports)
  pub needs_interop: bool,
}

#[derive(Debug)]
pub struct LinkStageOutput {
  pub module_table: ModuleTable,
  pub entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
  pub sorted_modules: Vec<ModuleIdx>,
  pub metas: LinkingMetadataVec,
  pub symbol_db: SymbolRefDb,
  /// Per-module statement-info table moved through the Link driver from Scan.
  pub stmt_infos: IndexStmtInfos,
  pub runtime: RuntimeModuleBrief,
  pub diagnostics: Diagnostics,
  pub used_external_symbols: UsedExternalSymbols,
  /// See [`RetainedExportSymbols`]; empty until the generate stage projects it.
  pub retained_export_symbols: RetainedExportSymbols,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  pub safely_merge_cjs_ns_map: FxHashMap<ModuleIdx, SafelyMergeCjsNsInfo>,
  pub external_import_namespace_merger: FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>>,
  /// https://rollupjs.org/plugin-development/#this-emitfile
  /// Used to store `preserveSignature` specified with `this.emitFile` in plugins.
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
  pub global_constant_symbol_map: FxHashMap<SymbolRef, ConstExportMeta>,
  pub normal_symbol_exports_chain_map: FxHashMap<SymbolRef, Vec<SymbolRef>>,
  pub(crate) lazy_json_export_initializers: LazyJsonExportInitializers,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
  /// True if any module has enum member values to inline. Computed once to avoid
  /// repeated full module table scans.
  pub has_enum_inlining: bool,
}

#[derive(Debug)]
pub struct LinkStage<'a> {
  scan_stage_output: NormalizedScanStageOutput,
  options: &'a SharedOptions,
}

impl<'a> LinkStage<'a> {
  pub fn new(scan_stage_output: NormalizedScanStageOutput, options: &'a SharedOptions) -> Self {
    Self { scan_stage_output, options }
  }

  #[expect(clippy::too_many_lines, reason = "the explicit pass order is the typed Link driver")]
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn link(self) -> (LinkStageOutput, IndexEcmaAst, UsedSymbolRefsBuilder) {
    let LinkStage { scan_stage_output, options } = self;
    let NormalizedScanStageOutput {
      module_table,
      index_ecma_ast,
      stmt_infos,
      entry_points,
      symbol_ref_db,
      runtime,
      warnings,
      dynamic_import_exports_usage_map,
      overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids,
      flat_options,
      user_defined_entry_modules,
      tla_module_count,
      tla_keyword_span_map,
    } = scan_stage_output;
    let mut diagnostics = Diagnostics::from(warnings);
    let tla_scan_facts = TlaScanFacts::new(tla_module_count, tla_keyword_span_map);
    let mut pass_pipeline = PassPipelineCtx::new();
    let (
      module_table,
      entry_plan,
      global_constants,
      dependencies,
      external_star_exports,
      execution_orders,
      sorted_modules,
    ) = {
      let (_, (module_table, global_constants)) = run_infallible_pass(
        ExtractGlobalConstantsPass,
        &mut pass_pipeline,
        ConstantExtractionInput { enabled: options.optimization.is_inline_const_enabled() },
        module_table,
      );

      let (_, entry_plan) = run_infallible_pass(
        CanonicalizeEntriesPass,
        &mut pass_pipeline,
        &module_table,
        entry_points,
      );
      let (_, dependencies) =
        run_infallible_pass(CollectInitialDependenciesPass, &mut pass_pipeline, &module_table, ());
      let (_, external_star_exports) =
        run_infallible_pass(CollectExternalStarExportsPass, &mut pass_pipeline, &module_table, ());
      let (execution_orders, sorted_modules) = run_infallible_pass(
        ComputeModuleExecutionOrderPass,
        &mut pass_pipeline,
        ComputeModuleExecutionOrderInput {
          module_table: &module_table,
          entry_plan: &entry_plan,
          runtime: runtime.id(),
          code_splitting_disabled: options.code_splitting.is_disabled(),
          check_circular_dependencies: options
            .checks
            .contains(EventKindSwitcher::CircularDependency),
        },
        (),
      );
      (
        module_table,
        entry_plan,
        global_constants,
        dependencies,
        external_star_exports,
        execution_orders,
        sorted_modules,
      )
    };
    let (tla_facts, ()) =
      run_infallible_pass(ComputeTlaPass, &mut pass_pipeline, &module_table, tla_scan_facts);

    let (_, (module_formats, wrapper_seeds)) = run_infallible_pass(
      DetermineModuleFormatsPass,
      &mut pass_pipeline,
      DetermineModuleFormatsInput {
        module_table: &module_table,
        entry_plan: &entry_plan,
        output_format: options.format,
        code_splitting_disabled: options.code_splitting.is_disabled(),
      },
      (),
    );
    let (_, cjs_namespace_merges) = run_infallible_pass(
      ComputeCjsNamespaceMergesPass,
      &mut pass_pipeline,
      ComputeCjsNamespaceMergesInput {
        module_table: &module_table,
        module_formats: &module_formats,
        strict_execution_order: options.is_strict_execution_order_enabled(),
      },
      (),
    );
    let (dynamic_exports, ()) = run_infallible_pass(
      ComputeDynamicExportsPass,
      &mut pass_pipeline,
      ComputeDynamicExportsInput { module_table: &module_table, module_formats: &module_formats },
      (),
    );
    let (_, wrapper_plan) = run_infallible_pass(
      PlanModuleWrappingPass,
      &mut pass_pipeline,
      PlanModuleWrappingInput {
        module_table: &module_table,
        module_formats: &module_formats,
        runtime: runtime.id(),
        strict_execution_order: options.is_strict_execution_order_enabled(),
        on_demand_wrapping: options.experimental.is_on_demand_wrapping_enabled(),
      },
      wrapper_seeds,
    );
    let (commonjs_helper, esm_helper) = if options.profiler_names {
      (runtime.resolve_symbol("__commonJS"), runtime.resolve_symbol("__esm"))
    } else {
      (runtime.resolve_symbol("__commonJSMin"), runtime.resolve_symbol("__esmMin"))
    };
    let (_, wrapper_output) = run_infallible_pass(
      CreateWrapperDeclarationsPass,
      &mut pass_pipeline,
      CreateWrapperDeclarationsInput { module_table: &module_table, commonjs_helper, esm_helper },
      CreateWrapperDeclarationsOwned { wrapper_plan, symbols: symbol_ref_db, stmt_infos },
    );
    let CreateWrapperDeclarationsOutput { wrapper_declarations, symbols, stmt_infos } =
      wrapper_output;
    let (_, normalized) = run_infallible_pass(
      NormalizeLazyExportsPass,
      &mut pass_pipeline,
      NormalizeLazyExportsInput {
        entry_plan: &entry_plan,
        cjs_namespace_merges: &cjs_namespace_merges,
        global_constants: &global_constants,
      },
      NormalizeLazyExportsOwned {
        module_table,
        ast_table: index_ecma_ast,
        stmt_infos,
        symbols,
        module_formats,
        wrapper_declarations,
      },
    );
    let NormalizeLazyExportsOutput {
      module_table,
      ast_table,
      stmt_infos,
      symbols,
      module_formats,
      module_wrappers,
      lazy_json_export_initializers,
      non_splittable_json_defaults,
    } = normalized;
    let (module_side_effects, ()) = run_infallible_pass(
      DetermineModuleSideEffectsPass,
      &mut pass_pipeline,
      DetermineModuleSideEffectsInput {
        module_table: &module_table,
        dynamic_exports: &dynamic_exports,
        module_wrappers: &module_wrappers,
      },
      (),
    );
    let (_, resolved_exports_draft) =
      run_infallible_pass(CollectResolvedExportsPass, &mut pass_pipeline, &module_table, ());

    let (_, binding) = run_infallible_pass(
      BindImportsPass,
      &mut pass_pipeline,
      BindImportsInput {
        module_table: &module_table,
        resolved_exports: &resolved_exports_draft,
        module_formats: &module_formats,
        dynamic_exports: &dynamic_exports,
        module_side_effects: &module_side_effects,
        execution_orders: &execution_orders,
        output_format: options.format,
        shim_missing_exports: options.shim_missing_exports,
      },
      BindImportsOwned { symbols, dependencies },
    );
    let BindImportsOutput {
      symbols,
      dependencies,
      shimmed_missing_exports,
      included_commonjs_export_symbols,
      normal_export_chains,
      external_namespace_merges,
    } = binding;
    let (_, resolved_exports) = run_infallible_pass(
      FinalizeResolvedExportsPass,
      &mut pass_pipeline,
      &symbols,
      resolved_exports_draft,
    );
    let (_, cjs_routing) = run_infallible_pass(
      ComputeCjsRoutingPass,
      &mut pass_pipeline,
      ComputeCjsRoutingInput {
        module_table: &module_table,
        module_formats: &module_formats,
        dynamic_exports: &dynamic_exports,
      },
      (),
    );
    let (_, member_resolution) = run_infallible_pass(
      ResolveMemberExpressionsPass,
      &mut pass_pipeline,
      ResolveMemberExpressionsInput {
        module_table: &module_table,
        stmt_infos: &stmt_infos,
        symbols: &symbols,
        resolved_exports: &resolved_exports,
        normal_export_chains: &normal_export_chains,
        module_side_effects: &module_side_effects,
        dynamic_exports: &dynamic_exports,
      },
      ResolveMemberExpressionsOwned {
        cjs_routing,
        non_splittable_json_defaults,
        global_constants,
        dependencies,
      },
    );
    let ResolveMemberExpressionsOutput { resolutions, cjs_routing, global_constants, dependencies } =
      member_resolution;
    let (_, entry_export_roots) = run_infallible_pass(
      CollectEntryExportRootsPass,
      &mut pass_pipeline,
      CollectEntryExportRootsInput {
        module_table: &module_table,
        entry_plan: &entry_plan,
        module_wrappers: &module_wrappers,
        resolved_exports: &resolved_exports,
        dynamic_import_usage: &dynamic_import_exports_usage_map,
        preserve_signature_overrides: &overrode_preserve_entry_signature_map,
        default_preserve_signature: options.preserve_entry_signatures,
      },
      (),
    );
    let (_, stmt_infos) = run_infallible_pass(
      CreateSyntheticExportStatementsPass,
      &mut pass_pipeline,
      CreateSyntheticExportStatementsInput {
        module_table: &module_table,
        module_formats: &module_formats,
        resolved_exports: &resolved_exports,
        shimmed_missing_exports: &shimmed_missing_exports,
        external_star_exports: &external_star_exports,
        export_all_helper: runtime.resolve_symbol("__exportAll"),
        re_export_helper: runtime.resolve_symbol("__reExport"),
        output_format: options.format,
        generated_code_symbols: options.generated_code.symbols,
      },
      stmt_infos,
    );
    let runtime_require_ref = (options.format.should_call_runtime_require()
      && options.polyfill_require_for_esm_format_with_node_platform())
    .then(|| runtime.resolve_symbol("__require"));
    let (statement_runtime_requirements, reference_output) = run_infallible_pass(
      ReferenceNeededSymbolsPass,
      &mut pass_pipeline,
      ReferenceNeededSymbolsInput {
        module_table: &module_table,
        module_formats: &module_formats,
        module_wrappers: &module_wrappers,
        dynamic_exports: &dynamic_exports,
        module_side_effects: &module_side_effects,
        cjs_namespace_merges: &cjs_namespace_merges,
        runtime_require_ref,
        output_format: options.format,
        chunking: ReferenceChunkingOptions {
          dynamic_import_in_cjs: options.dynamic_import_in_cjs,
          code_splitting_disabled: options.code_splitting.is_disabled(),
        },
        tree_shaking: ReferenceTreeShakingOptions {
          keep_names: options.keep_names,
          commonjs_treeshake: options.treeshake.commonjs(),
        },
      },
      ReferenceNeededSymbolsOwned { symbols, stmt_infos },
    );
    let ReferenceNeededSymbolsOutput { symbols, stmt_infos, import_record_patches } =
      reference_output;
    let (unreachable_dynamic_imports, cross_module_output) = run_infallible_pass(
      CrossModuleOptimizationPass,
      &mut pass_pipeline,
      CrossModuleOptimizationInput {
        module_table: &module_table,
        ast_table: &ast_table,
        symbols: &symbols,
        sorted_modules: &sorted_modules,
        entry_plan: &entry_plan,
        member_expr_resolutions: &resolutions,
        flat_options,
        options,
      },
      CrossModuleOptimizationOwned { stmt_infos, global_constants },
    );
    let CrossModuleOptimizationOutput { stmt_infos, global_constants } = cross_module_output;
    let (_, tree_shake_output) = run_infallible_pass(
      TreeShakePass,
      &mut pass_pipeline,
      TreeShakeInput {
        module_table: &module_table,
        stmt_infos: &stmt_infos,
        symbols: &symbols,
        runtime: &runtime,
        module_formats: &module_formats,
        module_wrappers: &module_wrappers,
        resolved_exports: &resolved_exports,
        cjs_routing: &cjs_routing,
        dependencies: &dependencies,
        member_expr_resolutions: &resolutions,
        module_side_effects: &module_side_effects,
        global_constants: &global_constants,
        entry_export_roots: &entry_export_roots,
        normal_export_chains: &normal_export_chains,
        included_commonjs_export_symbols: &included_commonjs_export_symbols,
        dynamic_import_usage: &dynamic_import_exports_usage_map,
        statement_runtime_requirements: &statement_runtime_requirements,
        unreachable_dynamic_imports: &unreachable_dynamic_imports,
        options: TreeShakeOptions {
          inclusion: TreeShakeInclusionPolicy {
            tree_shaking_enabled: options.treeshake.is_some(),
            commonjs_tree_shaking: options.treeshake.commonjs(),
            property_write_side_effects: options.treeshake.property_write_side_effects(),
            inline_const_smart: options.optimization.is_inline_const_smart_mode(),
          },
          preserve_modules: options.preserve_modules,
          dev_mode: options.is_dev_mode_enabled(),
          code_splitting_disabled: options.code_splitting.is_disabled(),
        },
      },
      TreeShakeOwned { entry_plan },
    );
    let TreeShakeOutput {
      retained_entries,
      inclusion,
      runtime_requirements,
      used_symbol_refs,
      used_external_symbols,
      module_patches,
      enum_inlining,
    } = tree_shake_output;
    drop((unreachable_dynamic_imports, statement_runtime_requirements));
    let (_, finalized_dependencies) = run_infallible_pass(
      FinalizeModuleDependenciesPass,
      &mut pass_pipeline,
      FinalizeModuleDependenciesInput {
        module_table: &module_table,
        stmt_infos: &stmt_infos,
        symbols: &symbols,
        inclusion: &inclusion,
        member_expr_resolutions: &resolutions,
        module_side_effects: &module_side_effects,
        retained_entries: &retained_entries,
        entry_export_roots: &entry_export_roots,
        runtime_idx: runtime.id(),
        tree_shaking_enabled: options.treeshake.is_some(),
      },
      FinalizeModuleDependenciesOwned { dependencies, runtime_requirements },
    );
    diagnostics.extend(pass_pipeline.into_diagnostics());

    LegacyOutputAdapter {
      execution_orders: &execution_orders,
      tla_facts: &tla_facts,
      module_formats,
      module_wrappers,
      dynamic_exports: &dynamic_exports,
      module_side_effects: &module_side_effects,
      resolved_exports,
      included_commonjs_export_symbols,
      cjs_routing,
      member_expr_resolutions: resolutions,
      shimmed_missing_exports,
      entry_export_roots,
      external_star_exports,
      inclusion,
      finalized_dependencies,
      retained_entries,
      sorted_modules,
      cjs_namespace_merges,
      external_namespace_merges: external_namespace_merges.into_inner(),
      global_constants,
      normal_export_chains,
      tree_shake_patches: module_patches,
      reference_patches: import_record_patches,
      used_symbol_refs,
      used_external_symbols,
      enum_inlining,
      lazy_json_export_initializers,
    }
    .finish(
      module_table,
      symbols,
      stmt_infos,
      runtime,
      diagnostics,
      ast_table,
      dynamic_import_exports_usage_map,
      overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids,
      user_defined_entry_modules,
    )
  }
}
