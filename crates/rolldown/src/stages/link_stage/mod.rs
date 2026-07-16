use arcstr::ArcStr;
#[cfg(debug_assertions)]
use rolldown_common::common_debug_symbol_ref;
use rolldown_common::{
  ConstExportMeta, EntryPoint, FlatOptions, ModuleIdx, ModuleTable, PreserveEntrySignatures,
  RetainedExportSymbols, RuntimeModuleBrief, SymbolRef, SymbolRefDb, UsedExternalSymbols,
  UsedSymbolRefsBuilder, dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_error::{Diagnostics, EventKindSwitcher};
use rolldown_utils::{
  indexmap::{FxIndexMap, FxIndexSet},
  pass::{PassPipelineCtx, Sealed, run_infallible_pass},
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
use non_splittable_json_defaults::NonSplittableJsonDefaults;

use passes::{
  BindImportsInput, BindImportsOutput, BindImportsOwned, BindImportsPass, CanonicalizeEntriesPass,
  CjsNamespaceMerges, CollectEntryExportRootsInput, CollectEntryExportRootsPass,
  CollectExternalStarExportsPass, CollectInitialDependenciesPass, CollectResolvedExportsPass,
  ComputeCjsNamespaceMergesInput, ComputeCjsNamespaceMergesPass, ComputeCjsRoutingInput,
  ComputeCjsRoutingPass, ComputeDynamicExportsInput, ComputeDynamicExportsPass,
  ComputeModuleExecutionOrderInput, ComputeModuleExecutionOrderPass, ComputeTlaPass,
  ConstantExtractionInput, CreateSyntheticExportStatementsInput,
  CreateSyntheticExportStatementsPass, CreateWrapperDeclarationsInput,
  CreateWrapperDeclarationsOutput, CreateWrapperDeclarationsOwned, CreateWrapperDeclarationsPass,
  CrossModuleOptimizationInput, CrossModuleOptimizationOutput, CrossModuleOptimizationOwned,
  CrossModuleOptimizationPass, DetermineModuleFormatsInput, DetermineModuleFormatsPass,
  DetermineModuleSideEffectsInput, DetermineModuleSideEffectsPass, DynamicExports,
  EntryExportRoots, EntryPlanDraft, ExternalStarExports, ExtractGlobalConstantsPass,
  FinalizeModuleDependenciesInput, FinalizeModuleDependenciesOwned, FinalizeModuleDependenciesPass,
  FinalizeResolvedExportsPass, GlobalConstants, GlobalConstantsDraft, MemberExprResolutions,
  ModuleFormats, ModuleSideEffects, ModuleWrappers, NormalizeLazyExportsInput,
  NormalizeLazyExportsOutput, NormalizeLazyExportsOwned, NormalizeLazyExportsPass,
  PlanModuleWrappingInput, PlanModuleWrappingPass, ReferenceChunkingOptions,
  ReferenceImportRecordPatches, ReferenceNeededSymbolsInput, ReferenceNeededSymbolsOutput,
  ReferenceNeededSymbolsOwned, ReferenceNeededSymbolsPass, ReferenceTreeShakingOptions,
  ResolveMemberExpressionsInput, ResolveMemberExpressionsOutput, ResolveMemberExpressionsOwned,
  ResolveMemberExpressionsPass, ResolvedExports, ShimmedMissingExports, SortedModules,
  StatementRuntimeRequirements, TlaScanFacts, TreeShakeInclusionPolicy, TreeShakeInput,
  TreeShakeOptions, TreeShakeOutput, TreeShakeOwned, TreeShakePass, UnreachableDynamicImports,
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
  /// Per-module statement-info table; see `LinkStage.stmt_infos`.
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
  pub module_table: ModuleTable,
  /// Raw scan entries, consumed by `CanonicalizeEntriesPass` at link entry.
  entry_points: Vec<EntryPoint>,
  pub symbols: SymbolRefDb,
  /// Per-module statement-info table. Detached from `EcmaView` at `LinkStage::new`
  /// (the field on `EcmaView` is left as an empty placeholder) so the parallel
  /// `ReferenceNeededSymbolsPass` can own and mutate disjoint slots through a
  /// zipped iterator. Threaded through `LinkStageOutput` to the generate stage
  /// and module finalizers, which used to read `module.stmt_infos` directly.
  pub stmt_infos: IndexStmtInfos,
  pub runtime: RuntimeModuleBrief,
  pub diagnostics: Diagnostics,
  pub ast_table: IndexEcmaAst,
  pub options: &'a SharedOptions,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
  pub flat_options: FlatOptions,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
  /// Scan-only TLA inputs. `ComputeTlaPass` consumes these at their only link use.
  tla_scan_facts: TlaScanFacts,
}

impl<'a> LinkStage<'a> {
  pub fn new(mut scan_stage_output: NormalizedScanStageOutput, options: &'a SharedOptions) -> Self {
    Self {
      // `stmt_infos` is produced by the scan stage on the side (in
      // `NormalizedScanStageOutput.stmt_infos`) rather than living on each
      // `EcmaView`, so we can move it directly here.
      stmt_infos: std::mem::take(&mut scan_stage_output.stmt_infos),
      module_table: scan_stage_output.module_table,
      entry_points: scan_stage_output.entry_points,
      symbols: scan_stage_output.symbol_ref_db,
      runtime: scan_stage_output.runtime,
      diagnostics: scan_stage_output.warnings.into(),
      ast_table: scan_stage_output.index_ecma_ast,
      dynamic_import_exports_usage_map: scan_stage_output.dynamic_import_exports_usage_map,
      options,
      overrode_preserve_entry_signature_map: scan_stage_output
        .overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids: scan_stage_output.entry_point_to_reference_ids,
      flat_options: scan_stage_output.flat_options,
      user_defined_entry_modules: scan_stage_output.user_defined_entry_modules,
      tla_scan_facts: TlaScanFacts::new(
        scan_stage_output.tla_module_count,
        scan_stage_output.tla_keyword_span_map,
      ),
    }
  }

  fn run_representation_and_side_effect_passes(
    &mut self,
    pass_pipeline: &mut PassPipelineCtx,
    entry_plan: &EntryPlanDraft,
    global_constants: &GlobalConstantsDraft,
  ) -> (
    LazyJsonExportInitializers,
    NonSplittableJsonDefaults,
    ModuleFormats,
    ModuleWrappers,
    Sealed<DynamicExports>,
    Sealed<ModuleSideEffects>,
    CjsNamespaceMerges,
  ) {
    let (_, (module_formats, wrapper_seeds)) = run_infallible_pass(
      DetermineModuleFormatsPass,
      pass_pipeline,
      DetermineModuleFormatsInput {
        module_table: &self.module_table,
        entry_plan,
        output_format: self.options.format,
        code_splitting_disabled: self.options.code_splitting.is_disabled(),
      },
      (),
    );
    let (_, cjs_namespace_merges) = run_infallible_pass(
      ComputeCjsNamespaceMergesPass,
      pass_pipeline,
      ComputeCjsNamespaceMergesInput {
        module_table: &self.module_table,
        module_formats: &module_formats,
        strict_execution_order: self.options.is_strict_execution_order_enabled(),
      },
      (),
    );
    let (dynamic_exports, ()) = run_infallible_pass(
      ComputeDynamicExportsPass,
      pass_pipeline,
      ComputeDynamicExportsInput {
        module_table: &self.module_table,
        module_formats: &module_formats,
      },
      (),
    );
    let (_, wrapper_plan) = run_infallible_pass(
      PlanModuleWrappingPass,
      pass_pipeline,
      PlanModuleWrappingInput {
        module_table: &self.module_table,
        module_formats: &module_formats,
        runtime: self.runtime.id(),
        strict_execution_order: self.options.is_strict_execution_order_enabled(),
        on_demand_wrapping: self.options.experimental.is_on_demand_wrapping_enabled(),
      },
      wrapper_seeds,
    );
    let (commonjs_helper, esm_helper) = if self.options.profiler_names {
      (self.runtime.resolve_symbol("__commonJS"), self.runtime.resolve_symbol("__esm"))
    } else {
      (self.runtime.resolve_symbol("__commonJSMin"), self.runtime.resolve_symbol("__esmMin"))
    };
    let (_, wrapper_output) = run_infallible_pass(
      CreateWrapperDeclarationsPass,
      pass_pipeline,
      CreateWrapperDeclarationsInput {
        module_table: &self.module_table,
        commonjs_helper,
        esm_helper,
      },
      CreateWrapperDeclarationsOwned {
        wrapper_plan,
        symbols: std::mem::take(&mut self.symbols),
        stmt_infos: std::mem::take(&mut self.stmt_infos),
      },
    );
    let CreateWrapperDeclarationsOutput { wrapper_declarations, symbols, stmt_infos } =
      wrapper_output;
    let (_, normalized) = run_infallible_pass(
      NormalizeLazyExportsPass,
      pass_pipeline,
      NormalizeLazyExportsInput {
        entry_plan,
        cjs_namespace_merges: &cjs_namespace_merges,
        global_constants,
      },
      NormalizeLazyExportsOwned {
        module_table: std::mem::take(&mut self.module_table),
        ast_table: std::mem::take(&mut self.ast_table),
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
    self.module_table = module_table;
    self.ast_table = ast_table;
    self.stmt_infos = stmt_infos;
    self.symbols = symbols;
    let (module_side_effects, ()) = run_infallible_pass(
      DetermineModuleSideEffectsPass,
      pass_pipeline,
      DetermineModuleSideEffectsInput {
        module_table: &self.module_table,
        dynamic_exports: &dynamic_exports,
        module_wrappers: &module_wrappers,
      },
      (),
    );
    (
      lazy_json_export_initializers,
      non_splittable_json_defaults,
      module_formats,
      module_wrappers,
      dynamic_exports,
      module_side_effects,
      cjs_namespace_merges,
    )
  }

  fn run_collect_entry_export_roots_pass(
    &self,
    pass_pipeline: &mut PassPipelineCtx,
    entry_plan: &EntryPlanDraft,
    module_wrappers: &ModuleWrappers,
    resolved_exports: &ResolvedExports,
  ) -> EntryExportRoots {
    let (_, entry_export_roots) = run_infallible_pass(
      CollectEntryExportRootsPass,
      pass_pipeline,
      CollectEntryExportRootsInput {
        module_table: &self.module_table,
        entry_plan,
        module_wrappers,
        resolved_exports,
        dynamic_import_usage: &self.dynamic_import_exports_usage_map,
        preserve_signature_overrides: &self.overrode_preserve_entry_signature_map,
        default_preserve_signature: self.options.preserve_entry_signatures,
      },
      (),
    );
    entry_export_roots
  }

  fn run_create_synthetic_export_statements_pass(
    &mut self,
    pass_pipeline: &mut PassPipelineCtx,
    module_formats: &ModuleFormats,
    resolved_exports: &ResolvedExports,
    shimmed_missing_exports: &ShimmedMissingExports,
    external_star_exports: &ExternalStarExports,
  ) {
    let (_, stmt_infos) = run_infallible_pass(
      CreateSyntheticExportStatementsPass,
      pass_pipeline,
      CreateSyntheticExportStatementsInput {
        module_table: &self.module_table,
        module_formats,
        resolved_exports,
        shimmed_missing_exports,
        external_star_exports,
        export_all_helper: self.runtime.resolve_symbol("__exportAll"),
        re_export_helper: self.runtime.resolve_symbol("__reExport"),
        output_format: self.options.format,
        generated_code_symbols: self.options.generated_code.symbols,
      },
      std::mem::take(&mut self.stmt_infos),
    );
    self.stmt_infos = stmt_infos;
  }

  fn run_reference_needed_symbols_pass(
    &mut self,
    pass_pipeline: &mut PassPipelineCtx,
    module_formats: &ModuleFormats,
    module_wrappers: &ModuleWrappers,
    dynamic_exports: &DynamicExports,
    module_side_effects: &ModuleSideEffects,
    cjs_namespace_merges: &CjsNamespaceMerges,
  ) -> (Sealed<StatementRuntimeRequirements>, ReferenceImportRecordPatches) {
    let runtime_require_ref = (self.options.format.should_call_runtime_require()
      && self.options.polyfill_require_for_esm_format_with_node_platform())
    .then(|| self.runtime.resolve_symbol("__require"));
    let (statement_runtime_requirements, reference_output) = run_infallible_pass(
      ReferenceNeededSymbolsPass,
      pass_pipeline,
      ReferenceNeededSymbolsInput {
        module_table: &self.module_table,
        module_formats,
        module_wrappers,
        dynamic_exports,
        module_side_effects,
        cjs_namespace_merges,
        runtime_require_ref,
        output_format: self.options.format,
        chunking: ReferenceChunkingOptions {
          dynamic_import_in_cjs: self.options.dynamic_import_in_cjs,
          code_splitting_disabled: self.options.code_splitting.is_disabled(),
        },
        tree_shaking: ReferenceTreeShakingOptions {
          keep_names: self.options.keep_names,
          commonjs_treeshake: self.options.treeshake.commonjs(),
        },
      },
      ReferenceNeededSymbolsOwned {
        symbols: std::mem::take(&mut self.symbols),
        stmt_infos: std::mem::take(&mut self.stmt_infos),
      },
    );
    let ReferenceNeededSymbolsOutput { symbols, stmt_infos, import_record_patches } =
      reference_output;
    self.symbols = symbols;
    self.stmt_infos = stmt_infos;
    (statement_runtime_requirements, import_record_patches)
  }

  fn run_cross_module_optimization_pass(
    &mut self,
    pass_pipeline: &mut PassPipelineCtx,
    sorted_modules: &SortedModules,
    entry_plan: &EntryPlanDraft,
    member_expr_resolutions: &MemberExprResolutions,
    global_constants: GlobalConstantsDraft,
  ) -> (Sealed<UnreachableDynamicImports>, GlobalConstants) {
    let (unreachable_dynamic_imports, output) = run_infallible_pass(
      CrossModuleOptimizationPass,
      pass_pipeline,
      CrossModuleOptimizationInput {
        module_table: &self.module_table,
        ast_table: &self.ast_table,
        symbols: &self.symbols,
        sorted_modules,
        entry_plan,
        member_expr_resolutions,
        flat_options: self.flat_options,
        options: self.options,
      },
      CrossModuleOptimizationOwned {
        stmt_infos: std::mem::take(&mut self.stmt_infos),
        global_constants,
      },
    );
    let CrossModuleOptimizationOutput { stmt_infos, global_constants } = output;
    self.stmt_infos = stmt_infos;
    (unreachable_dynamic_imports, global_constants)
  }

  #[expect(clippy::too_many_lines, reason = "the explicit pass order is the typed Link driver")]
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn link(mut self) -> (LinkStageOutput, IndexEcmaAst, UsedSymbolRefsBuilder) {
    let mut pass_pipeline = PassPipelineCtx::new();
    let (
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
        ConstantExtractionInput { enabled: self.options.optimization.is_inline_const_enabled() },
        std::mem::take(&mut self.module_table),
      );
      self.module_table = module_table;

      let (_, entry_plan) = run_infallible_pass(
        CanonicalizeEntriesPass,
        &mut pass_pipeline,
        &self.module_table,
        std::mem::take(&mut self.entry_points),
      );
      let (_, dependencies) = run_infallible_pass(
        CollectInitialDependenciesPass,
        &mut pass_pipeline,
        &self.module_table,
        (),
      );
      let (_, external_star_exports) = run_infallible_pass(
        CollectExternalStarExportsPass,
        &mut pass_pipeline,
        &self.module_table,
        (),
      );
      let (execution_orders, sorted_modules) = run_infallible_pass(
        ComputeModuleExecutionOrderPass,
        &mut pass_pipeline,
        ComputeModuleExecutionOrderInput {
          module_table: &self.module_table,
          entry_plan: &entry_plan,
          runtime: self.runtime.id(),
          code_splitting_disabled: self.options.code_splitting.is_disabled(),
          check_circular_dependencies: self
            .options
            .checks
            .contains(EventKindSwitcher::CircularDependency),
        },
        (),
      );
      (
        entry_plan,
        global_constants,
        dependencies,
        external_star_exports,
        execution_orders,
        sorted_modules,
      )
    };
    let (tla_facts, ()) = run_infallible_pass(
      ComputeTlaPass,
      &mut pass_pipeline,
      &self.module_table,
      std::mem::take(&mut self.tla_scan_facts),
    );
    let (
      lazy_json_export_initializers,
      non_splittable_json_defaults,
      module_formats,
      module_wrappers,
      dynamic_exports,
      module_side_effects,
      cjs_namespace_merges,
    ) = self.run_representation_and_side_effect_passes(
      &mut pass_pipeline,
      &entry_plan,
      &global_constants,
    );
    let (_, resolved_exports_draft) =
      run_infallible_pass(CollectResolvedExportsPass, &mut pass_pipeline, &self.module_table, ());

    let (_, binding) = run_infallible_pass(
      BindImportsPass,
      &mut pass_pipeline,
      BindImportsInput {
        module_table: &self.module_table,
        resolved_exports: &resolved_exports_draft,
        module_formats: &module_formats,
        dynamic_exports: &dynamic_exports,
        module_side_effects: &module_side_effects,
        execution_orders: &execution_orders,
        output_format: self.options.format,
        shim_missing_exports: self.options.shim_missing_exports,
      },
      BindImportsOwned { symbols: std::mem::take(&mut self.symbols), dependencies },
    );
    let BindImportsOutput {
      symbols,
      dependencies,
      shimmed_missing_exports,
      included_commonjs_export_symbols,
      normal_export_chains,
      external_namespace_merges,
    } = binding;
    self.symbols = symbols;

    let (_, resolved_exports) = run_infallible_pass(
      FinalizeResolvedExportsPass,
      &mut pass_pipeline,
      &self.symbols,
      resolved_exports_draft,
    );
    let (_, cjs_routing) = run_infallible_pass(
      ComputeCjsRoutingPass,
      &mut pass_pipeline,
      ComputeCjsRoutingInput {
        module_table: &self.module_table,
        module_formats: &module_formats,
        dynamic_exports: &dynamic_exports,
      },
      (),
    );
    let (_, member_resolution) = run_infallible_pass(
      ResolveMemberExpressionsPass,
      &mut pass_pipeline,
      ResolveMemberExpressionsInput {
        module_table: &self.module_table,
        stmt_infos: &self.stmt_infos,
        symbols: &self.symbols,
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
    let entry_export_roots = self.run_collect_entry_export_roots_pass(
      &mut pass_pipeline,
      &entry_plan,
      &module_wrappers,
      &resolved_exports,
    );
    self.run_create_synthetic_export_statements_pass(
      &mut pass_pipeline,
      &module_formats,
      &resolved_exports,
      &shimmed_missing_exports,
      &external_star_exports,
    );
    let (statement_runtime_requirements, import_record_patches) = self
      .run_reference_needed_symbols_pass(
        &mut pass_pipeline,
        &module_formats,
        &module_wrappers,
        &dynamic_exports,
        &module_side_effects,
        &cjs_namespace_merges,
      );
    let (unreachable_dynamic_imports, global_constants) = self.run_cross_module_optimization_pass(
      &mut pass_pipeline,
      &sorted_modules,
      &entry_plan,
      &resolutions,
      global_constants,
    );
    let (_, tree_shake_output) = run_infallible_pass(
      TreeShakePass,
      &mut pass_pipeline,
      TreeShakeInput {
        module_table: &self.module_table,
        stmt_infos: &self.stmt_infos,
        symbols: &self.symbols,
        runtime: &self.runtime,
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
        dynamic_import_usage: &self.dynamic_import_exports_usage_map,
        statement_runtime_requirements: &statement_runtime_requirements,
        unreachable_dynamic_imports: &unreachable_dynamic_imports,
        options: TreeShakeOptions {
          inclusion: TreeShakeInclusionPolicy {
            tree_shaking_enabled: self.options.treeshake.is_some(),
            commonjs_tree_shaking: self.options.treeshake.commonjs(),
            property_write_side_effects: self.options.treeshake.property_write_side_effects(),
            inline_const_smart: self.options.optimization.is_inline_const_smart_mode(),
          },
          preserve_modules: self.options.preserve_modules,
          dev_mode: self.options.is_dev_mode_enabled(),
          code_splitting_disabled: self.options.code_splitting.is_disabled(),
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
        module_table: &self.module_table,
        stmt_infos: &self.stmt_infos,
        symbols: &self.symbols,
        inclusion: &inclusion,
        member_expr_resolutions: &resolutions,
        module_side_effects: &module_side_effects,
        retained_entries: &retained_entries,
        entry_export_roots: &entry_export_roots,
        runtime_idx: self.runtime.id(),
        tree_shaking_enabled: self.options.treeshake.is_some(),
      },
      FinalizeModuleDependenciesOwned { dependencies, runtime_requirements },
    );
    self.diagnostics.extend(pass_pipeline.into_diagnostics());

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
    .finish(self)
  }

  /// A helper function used to debug symbol in link process
  /// given any `SymbolRef` the function will return the string representation of the symbol
  /// format: `${stable_id} -> ${symbol_name}`
  #[cfg(debug_assertions)]
  #[cfg_attr(debug_assertions, expect(unused))]
  pub fn debug_symbol_ref(&self, symbol_ref: SymbolRef) -> String {
    common_debug_symbol_ref(symbol_ref, &self.module_table.modules, &self.symbols)
  }
}
