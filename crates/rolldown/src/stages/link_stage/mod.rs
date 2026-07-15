use arcstr::ArcStr;
use oxc_index::IndexVec;
#[cfg(debug_assertions)]
use rolldown_common::common_debug_symbol_ref;
use rolldown_common::{
  ConstExportMeta, DependedRuntimeHelperMap, EntryPoint, FlatOptions, Module, ModuleIdx,
  ModuleTable, PreserveEntrySignatures, RetainedExportSymbols, RuntimeModuleBrief, SymbolRef,
  SymbolRefDb, UsedExternalSymbols, UsedSymbolRefsBuilder,
  dynamic_import_usage::DynamicImportExportsUsage,
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
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

use super::scan_stage::NormalizedScanStageOutput;

mod bind_imports_and_exports;
mod create_exports_for_ecma_modules;
mod cross_module_optimization;
mod generate_lazy_export;
pub mod lazy_json_export_initializers;
mod non_splittable_json_defaults;
mod passes;
mod patch_module_dependencies;
mod reference_needed_symbols;
#[cfg(feature = "testing")]
pub mod testing;
mod tree_shaking;

pub use tree_shaking::{
  IncludeContext, ModuleInclusionVec, ModuleNamespaceReasonVec, StmtInclusionVec,
  SymbolIncludeReason, compute_body_demand_keys, include_runtime_symbol, include_symbol,
};

use lazy_json_export_initializers::LazyJsonExportInitializers;
use non_splittable_json_defaults::NonSplittableJsonDefaults;

use passes::{
  CanonicalizeEntriesPass, CollectExternalStarExportsPass, CollectInitialDependenciesPass,
  ComputeCjsNamespaceMergesInput, ComputeCjsNamespaceMergesPass, ComputeDynamicExportsInput,
  ComputeDynamicExportsPass, ComputeModuleExecutionOrderInput, ComputeModuleExecutionOrderPass,
  ComputeTlaPass, ConstantExtractionInput, CreateWrapperDeclarationsInput,
  CreateWrapperDeclarationsOutput, CreateWrapperDeclarationsOwned, CreateWrapperDeclarationsPass,
  DetermineModuleFormatsInput, DetermineModuleFormatsPass, DetermineModuleSideEffectsInput,
  DetermineModuleSideEffectsPass, DynamicExports, EntryPlanDraft, ExtractGlobalConstantsPass,
  GlobalConstantsDraft, ModuleFormats, ModuleSideEffects, ModuleWrappers,
  NormalizeLazyExportsInput, NormalizeLazyExportsOutput, NormalizeLazyExportsOwned,
  NormalizeLazyExportsPass, PlanModuleWrappingInput, PlanModuleWrappingPass, TlaScanFacts,
  WrapperDeclaration,
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
  pub entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
  pub symbols: SymbolRefDb,
  /// Per-module runtime-helper-dependency map. Detached from `EcmaView` so the
  /// parallel walk in `reference_needed_symbols` can mutate it through `&mut`
  /// from a zipped iterator without aliasing tricks.
  pub depended_runtime_helper: IndexVec<ModuleIdx, Box<DependedRuntimeHelperMap>>,
  /// Per-module statement-info table. Detached from `EcmaView` at `LinkStage::new`
  /// (the field on `EcmaView` is left as an empty placeholder) so the parallel
  /// walk in `reference_needed_symbols` can mutate it through `&mut` from a
  /// zipped iterator without aliasing tricks. Threaded through `LinkStageOutput`
  /// to the generate stage and module finalizers, which used to read
  /// `module.stmt_infos` directly.
  pub stmt_infos: IndexStmtInfos,
  pub runtime: RuntimeModuleBrief,
  pub sorted_modules: Vec<ModuleIdx>,
  pub metas: LinkingMetadataVec,
  pub diagnostics: Diagnostics,
  pub ast_table: IndexEcmaAst,
  pub options: &'a SharedOptions,
  pub used_symbol_refs: UsedSymbolRefsBuilder,
  pub used_external_symbols: UsedExternalSymbols,
  pub safely_merge_cjs_ns_map: FxHashMap<ModuleIdx, SafelyMergeCjsNsInfo>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  pub normal_symbol_exports_chain_map: FxHashMap<SymbolRef, Vec<SymbolRef>>,
  pub external_import_namespace_merger: FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>>,
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
  pub global_constant_symbol_map: FxHashMap<SymbolRef, ConstExportMeta>,
  pub flat_options: FlatOptions,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
  /// Scan-only TLA inputs. `ComputeTlaPass` consumes these at their only link use.
  tla_scan_facts: TlaScanFacts,
  /// Computed during `include_statements`, reused when building `LinkStageOutput`.
  pub has_enum_inlining: bool,
}

impl<'a> LinkStage<'a> {
  pub fn new(mut scan_stage_output: NormalizedScanStageOutput, options: &'a SharedOptions) -> Self {
    Self {
      sorted_modules: Vec::new(),
      global_constant_symbol_map: FxHashMap::default(),
      depended_runtime_helper: scan_stage_output
        .module_table
        .modules
        .iter()
        .map(|_| Box::default())
        .collect::<IndexVec<ModuleIdx, _>>(),
      // `stmt_infos` is produced by the scan stage on the side (in
      // `NormalizedScanStageOutput.stmt_infos`) rather than living on each
      // `EcmaView`, so we can move it directly here.
      stmt_infos: std::mem::take(&mut scan_stage_output.stmt_infos),
      metas: scan_stage_output
        .module_table
        .modules
        .iter()
        .map(|_| LinkingMetadata::default())
        .collect::<IndexVec<ModuleIdx, _>>(),
      module_table: scan_stage_output.module_table,
      entry_points: scan_stage_output.entry_points,
      entries: FxIndexMap::default(),
      symbols: scan_stage_output.symbol_ref_db,
      runtime: scan_stage_output.runtime,
      diagnostics: scan_stage_output.warnings.into(),
      ast_table: scan_stage_output.index_ecma_ast,
      dynamic_import_exports_usage_map: scan_stage_output.dynamic_import_exports_usage_map,
      options,
      used_symbol_refs: UsedSymbolRefsBuilder::default(),
      used_external_symbols: UsedExternalSymbols::default(),
      safely_merge_cjs_ns_map: FxHashMap::default(),
      normal_symbol_exports_chain_map: FxHashMap::default(),
      external_import_namespace_merger: FxHashMap::default(),
      overrode_preserve_entry_signature_map: scan_stage_output
        .overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids: scan_stage_output.entry_point_to_reference_ids,
      flat_options: scan_stage_output.flat_options,
      user_defined_entry_modules: scan_stage_output.user_defined_entry_modules,
      tla_scan_facts: TlaScanFacts::new(
        scan_stage_output.tla_module_count,
        scan_stage_output.tla_keyword_span_map,
      ),
      has_enum_inlining: false,
    }
  }

  fn run_representation_and_side_effect_passes(
    &mut self,
    pass_pipeline: &mut PassPipelineCtx,
    entry_plan: &EntryPlanDraft,
    global_constants: &GlobalConstantsDraft,
  ) -> (LazyJsonExportInitializers, NonSplittableJsonDefaults) {
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
    self.project_module_side_effects(&module_side_effects);
    drop(module_side_effects);
    self.project_representation_results(module_formats, module_wrappers, &dynamic_exports);
    self.safely_merge_cjs_ns_map = cjs_namespace_merges.into_legacy();
    (lazy_json_export_initializers, non_splittable_json_defaults)
  }

  fn project_module_side_effects(&mut self, module_side_effects: &ModuleSideEffects) {
    if module_side_effects.module_count() != self.module_table.modules.len() {
      tracing::error!(
        side_effects = module_side_effects.module_count(),
        modules = self.module_table.modules.len(),
        "module-side-effect layout mismatch"
      );
    }
    for index in 0..module_side_effects.module_count() {
      let module_idx = ModuleIdx::new(index);
      let side_effects = module_side_effects.get(module_idx);
      match self.module_table.modules.get_mut(module_idx) {
        Some(Module::Normal(module)) => module.side_effects = side_effects,
        Some(Module::External(_)) => {}
        None => tracing::error!(
          module = module_idx.index(),
          "module-side-effect result targets a missing module"
        ),
      }
    }
  }

  fn project_representation_results(
    &mut self,
    module_formats: ModuleFormats,
    module_wrappers: ModuleWrappers,
    dynamic_exports: &DynamicExports,
  ) {
    if module_formats.module_count() != self.module_table.modules.len() {
      tracing::error!(
        formats = module_formats.module_count(),
        modules = self.module_table.modules.len(),
        "module-format layout mismatch"
      );
    }
    for (module_idx, exports_kind) in module_formats.into_normal_modules() {
      match self.module_table.modules.get_mut(module_idx) {
        Some(Module::Normal(module)) => module.exports_kind = exports_kind,
        Some(Module::External(_)) | None => tracing::error!(
          module = module_idx.index(),
          "normal module format targets a missing or external module"
        ),
      }
    }
    for module_idx in dynamic_exports.modules() {
      self.metas[module_idx].has_dynamic_exports = true;
    }
    for (module_idx, declaration, required_by_other_module) in module_wrappers.into_modules() {
      let meta = &mut self.metas[module_idx];
      meta.required_by_other_module = required_by_other_module;
      match declaration {
        WrapperDeclaration::None => meta.set_wrap_kind(rolldown_common::WrapKind::None),
        WrapperDeclaration::Cjs { wrapper_ref, wrapper_stmt_info } => {
          meta.set_wrap_kind(rolldown_common::WrapKind::Cjs);
          meta.wrapper_ref = Some(wrapper_ref);
          meta.wrapper_stmt_info = Some(wrapper_stmt_info);
        }
        WrapperDeclaration::Esm { wrapper_ref, wrapper_stmt_info } => {
          meta.set_wrap_kind(rolldown_common::WrapKind::Esm);
          meta.wrapper_ref = Some(wrapper_ref);
          meta.wrapper_stmt_info = Some(wrapper_stmt_info);
        }
      }
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub fn link(mut self) -> (LinkStageOutput, IndexEcmaAst, UsedSymbolRefsBuilder) {
    let mut pass_pipeline = PassPipelineCtx::new();
    let (entry_plan, global_constants) = {
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
      for (module_idx, module_dependencies) in dependencies.into_inner().into_iter_enumerated() {
        self.metas[module_idx].dependencies = module_dependencies;
      }
      let (_, external_star_exports) = run_infallible_pass(
        CollectExternalStarExportsPass,
        &mut pass_pipeline,
        &self.module_table,
        (),
      );
      for (module_idx, record_ids) in external_star_exports.into_inner().into_iter_enumerated() {
        self.metas[module_idx].star_exports_from_external_modules = record_ids;
      }
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
      for (module_idx, exec_order) in execution_orders.assigned() {
        match &mut self.module_table[module_idx] {
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
      self.sorted_modules = sorted_modules.into_inner();
      (entry_plan, global_constants)
    };
    {
      let (tla_facts, ()) = run_infallible_pass(
        ComputeTlaPass,
        &mut pass_pipeline,
        &self.module_table,
        std::mem::take(&mut self.tla_scan_facts),
      );
      debug_assert_eq!(tla_facts.module_count(), self.metas.len());
      for module_idx in tla_facts.modules() {
        self.metas[module_idx].is_tla_or_contains_tla_dependency = true;
      }
    }
    let (lazy_json_export_initializers, non_splittable_json_defaults) = self
      .run_representation_and_side_effect_passes(
        &mut pass_pipeline,
        &entry_plan,
        &global_constants,
      );
    self.entries = entry_plan.into_legacy_entries();
    self.global_constant_symbol_map = global_constants.into_legacy();
    self.diagnostics.extend(pass_pipeline.into_diagnostics());
    self.bind_imports_and_exports(&non_splittable_json_defaults);
    drop(non_splittable_json_defaults);
    self.create_exports_for_ecma_modules();
    self.reference_needed_symbols();
    let unreachable_import_expression_node_ids = self.cross_module_optimization();
    self.include_statements(&unreachable_import_expression_node_ids);
    self.patch_module_dependencies();

    tracing::trace!("meta {:#?}", self.metas.iter_enumerated().collect::<Vec<_>>());

    let output = LinkStageOutput {
      module_table: self.module_table,
      entries: self.entries,
      sorted_modules: self.sorted_modules,
      metas: self.metas,
      symbol_db: self.symbols,
      stmt_infos: self.stmt_infos,
      runtime: self.runtime,
      diagnostics: self.diagnostics,
      used_external_symbols: self.used_external_symbols,
      retained_export_symbols: RetainedExportSymbols::default(),
      dynamic_import_exports_usage_map: self.dynamic_import_exports_usage_map,
      safely_merge_cjs_ns_map: self.safely_merge_cjs_ns_map,
      external_import_namespace_merger: self.external_import_namespace_merger,
      overrode_preserve_entry_signature_map: self.overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids: self.entry_point_to_reference_ids,
      global_constant_symbol_map: self.global_constant_symbol_map,
      normal_symbol_exports_chain_map: self.normal_symbol_exports_chain_map,
      lazy_json_export_initializers,
      user_defined_entry_modules: self.user_defined_entry_modules,
      has_enum_inlining: self.has_enum_inlining,
    };
    #[cfg(feature = "testing")]
    let output = {
      let mut output = output;
      testing::observe_link_output(&mut output);
      output
    };
    (output, self.ast_table, self.used_symbol_refs)
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
