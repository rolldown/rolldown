//! Typed link passes and their narrow artifacts.

#![forbid(unsafe_code)]

mod bind_imports;
mod canonicalize_entries;
mod collect_entry_export_roots;
mod collect_external_star_exports;
mod collect_initial_dependencies;
mod collect_resolved_exports;
mod compute_cjs_namespace_merges;
mod compute_cjs_routing;
mod compute_dynamic_exports;
mod compute_module_execution_order;
mod compute_tla;
mod create_synthetic_export_statements;
mod create_wrapper_declarations;
mod cross_module_optimization;
mod determine_module_formats;
mod determine_module_side_effects;
mod extract_global_constants;
mod finalize_module_dependencies;
mod finalize_resolved_exports;
mod normalize_lazy_exports;
mod plan_module_wrapping;
mod reference_needed_symbols;
mod resolve_member_expressions;
mod tree_shake;

pub(super) use bind_imports::{
  BindImportsInput, BindImportsOutput, BindImportsOwned, IncludedCommonJsExportSymbols,
  NormalExportChains, ShimmedMissingExports,
};
pub(super) use canonicalize_entries::EntryPlanDraft;
pub(super) use collect_entry_export_roots::{CollectEntryExportRootsInput, EntryExportRoots};
pub(super) use collect_external_star_exports::ExternalStarExports;
pub(super) use collect_initial_dependencies::ModuleDependenciesDraft;
pub(super) use collect_resolved_exports::ResolvedExportsDraft;
pub(super) use compute_cjs_namespace_merges::{CjsNamespaceMerges, ComputeCjsNamespaceMergesInput};
pub(super) use compute_cjs_routing::{CjsRoutingDraft, CjsRoutingFinal, ComputeCjsRoutingInput};
pub(super) use compute_dynamic_exports::{ComputeDynamicExportsInput, DynamicExports};
pub(super) use compute_module_execution_order::{
  ComputeModuleExecutionOrderInput, ModuleExecutionOrders, SortedModules,
};
pub(super) use compute_tla::{TlaFacts, TlaScanFacts};
pub(super) use create_synthetic_export_statements::CreateSyntheticExportStatementsInput;
pub(super) use create_wrapper_declarations::{
  CreateWrapperDeclarationsInput, CreateWrapperDeclarationsOutput, CreateWrapperDeclarationsOwned,
  ModuleWrappers, WrapperDeclaration, WrapperDeclarationsDraft,
};
pub(super) use cross_module_optimization::{
  CrossModuleOptimizationInput, CrossModuleOptimizationOutput, CrossModuleOptimizationOwned,
  UnreachableDynamicImports,
};
pub(super) use determine_module_formats::{
  DetermineModuleFormatsInput, ModuleFormats, ModuleFormatsDraft,
};
pub(super) use determine_module_side_effects::{
  DetermineModuleSideEffectsInput, ModuleSideEffects,
};
pub(super) use extract_global_constants::{
  ConstantExtractionInput, GlobalConstants, GlobalConstantsDraft,
};
pub(super) use finalize_module_dependencies::{
  FinalizeModuleDependenciesInput, FinalizeModuleDependenciesOwned, FinalizedModuleDependencies,
};
pub(super) use finalize_resolved_exports::ResolvedExports;
pub(super) use normalize_lazy_exports::{
  NormalizeLazyExportsInput, NormalizeLazyExportsOutput, NormalizeLazyExportsOwned,
};
pub(super) use plan_module_wrapping::PlanModuleWrappingInput;
pub(super) use reference_needed_symbols::{
  ReferenceChunkingOptions, ReferenceImportRecordPatches, ReferenceNeededSymbolsInput,
  ReferenceNeededSymbolsOutput, ReferenceNeededSymbolsOwned, ReferenceTreeShakingOptions,
  StatementRuntimeRequirements,
};
pub(super) use resolve_member_expressions::{
  MemberExprResolutions, ResolveMemberExpressionsInput, ResolveMemberExpressionsOutput,
  ResolveMemberExpressionsOwned,
};
pub(super) use tree_shake::{
  EnumInliningPresence, InclusionResults, ModuleRuntimeRequirementsDraft, RetainedEntries,
  TreeShakeInclusionPolicy, TreeShakeInput, TreeShakeModulePatches, TreeShakeOptions,
  TreeShakeOutput, TreeShakeOwned,
};

#[derive(Clone, Copy)]
pub(super) struct BindImportsPass;

#[derive(Clone, Copy)]
pub(super) struct CanonicalizeEntriesPass;

#[derive(Clone, Copy)]
pub(super) struct CollectEntryExportRootsPass;

#[derive(Clone, Copy)]
pub(super) struct CollectExternalStarExportsPass;

#[derive(Clone, Copy)]
pub(super) struct CollectInitialDependenciesPass;

#[derive(Clone, Copy)]
pub(super) struct CollectResolvedExportsPass;

#[derive(Clone, Copy)]
pub(super) struct ComputeCjsRoutingPass;

#[derive(Clone, Copy)]
pub(super) struct ComputeCjsNamespaceMergesPass;

#[derive(Clone, Copy)]
pub(super) struct ComputeDynamicExportsPass;

#[derive(Clone, Copy)]
pub(super) struct ComputeModuleExecutionOrderPass;

#[derive(Clone, Copy)]
pub(super) struct ComputeTlaPass;

#[derive(Clone, Copy)]
pub(super) struct CrossModuleOptimizationPass;

#[derive(Clone, Copy)]
pub(super) struct CreateSyntheticExportStatementsPass;

#[derive(Clone, Copy)]
pub(super) struct CreateWrapperDeclarationsPass;

#[derive(Clone, Copy)]
pub(super) struct DetermineModuleFormatsPass;

#[derive(Clone, Copy)]
pub(super) struct DetermineModuleSideEffectsPass;

#[derive(Clone, Copy)]
pub(super) struct ExtractGlobalConstantsPass;

#[derive(Clone, Copy)]
pub(super) struct FinalizeResolvedExportsPass;

#[derive(Clone, Copy)]
pub(super) struct FinalizeModuleDependenciesPass;

#[derive(Clone, Copy)]
pub(super) struct NormalizeLazyExportsPass;

#[derive(Clone, Copy)]
pub(super) struct PlanModuleWrappingPass;

#[derive(Clone, Copy)]
pub(super) struct ReferenceNeededSymbolsPass;

#[derive(Clone, Copy)]
pub(super) struct ResolveMemberExpressionsPass;

#[derive(Clone, Copy)]
pub(super) struct TreeShakePass;

#[cfg(test)]
mod inventory;

#[cfg(test)]
pub(super) mod test_utils {
  use oxc::{semantic::SymbolId, span::Span};
  use oxc_index::IndexVec;
  use rolldown_common::{
    EcmaModuleAstUsage, EcmaView, EcmaViewMeta, EntryPoint, EntryPointKind, ExportsKind,
    ExternalModule, HmrInfo, ImportKind, Module, ModuleDefFormat, ModuleId, ModuleIdx, ModuleTable,
    RawImportRecord, ResolvedId, StableModuleId, SymbolRef, bundler_options::ModuleType,
    side_effects::DeterminedSideEffects,
  };
  use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
  use rustc_hash::{FxHashMap, FxHashSet};

  pub(in crate::stages::link_stage) type TestImport = (ImportKind, Option<usize>, Span);

  pub(in crate::stages::link_stage) fn module_idx(index: usize) -> ModuleIdx {
    ModuleIdx::from_usize(index)
  }

  fn symbol_ref(owner: ModuleIdx) -> SymbolRef {
    SymbolRef { owner, symbol: SymbolId::new(0) }
  }

  pub(in crate::stages::link_stage) fn normal_module(
    index: usize,
    has_tla: bool,
    imports: Vec<TestImport>,
  ) -> Module {
    normal_module_with_id(index, &format!("m{index}.js"), has_tla, imports)
  }

  pub(in crate::stages::link_stage) fn normal_module_with_id(
    index: usize,
    id: &str,
    has_tla: bool,
    imports: Vec<TestImport>,
  ) -> Module {
    let idx = module_idx(index);
    let id = ModuleId::new(id);
    let namespace_ref = symbol_ref(idx);
    let import_records = imports
      .into_iter()
      .map(|(kind, target, importer_span)| {
        RawImportRecord::new(
          format!("./m{}.js", target.unwrap_or(usize::MAX)).into(),
          kind,
          namespace_ref,
          importer_span,
          importer_span,
          None,
          None,
        )
        .into_resolved(target.map(module_idx))
      })
      .collect::<IndexVec<_, _>>();
    let mut ast_usage = EcmaModuleAstUsage::empty();
    ast_usage.set(EcmaModuleAstUsage::TopLevelAwait, has_tla);

    Module::normal(rolldown_common::NormalModule {
      exec_order: u32::MAX,
      idx,
      stable_id: StableModuleId::from_module_id(id.clone()),
      debug_id: id.to_string(),
      repr_name: id.to_string(),
      module_type: ModuleType::Js,
      ecma_view: EcmaView {
        dummy_record_set: FxHashSet::default(),
        source: " ".repeat(256).into(),
        def_format: ModuleDefFormat::EsmMjs,
        namespace_object_ref: namespace_ref,
        named_imports: FxIndexMap::default(),
        named_exports: FxHashMap::default(),
        import_records,
        imports: FxHashMap::default(),
        exports_kind: ExportsKind::Esm,
        default_export_ref: namespace_ref,
        sourcemap_chain: Vec::new(),
        importers: FxIndexSet::default(),
        importers_idx: FxIndexSet::default(),
        dynamic_importers: FxIndexSet::default(),
        dynamic_importers_idx: FxIndexSet::default(),
        imported_ids: FxIndexSet::default(),
        dynamically_imported_ids: FxIndexSet::default(),
        side_effects: DeterminedSideEffects::Analyzed(false),
        ast_usage,
        self_referenced_class_decl_symbol_ids: FxHashSet::default(),
        hashbang_range: None,
        directive_range: Vec::new(),
        meta: EcmaViewMeta::empty(),
        mutations: Vec::new(),
        new_url_references: FxHashMap::default(),
        this_expr_replace_map: FxHashMap::default(),
        hmr_hot_ref: None,
        hmr_info: HmrInfo::default(),
        constant_export_map: FxHashMap::default(),
        enum_member_value_map: FxHashMap::default(),
        import_attribute_map: FxHashMap::default(),
        json_module_none_self_reference_included_symbol: None,
        cjs_reexport_import_record_ids: Vec::new(),
      },
      originative_resolved_id: ResolvedId { id: id.clone(), ..ResolvedId::default() },
      id,
    })
  }

  pub(in crate::stages::link_stage) fn external_module(index: usize, id: &str) -> Module {
    let idx = module_idx(index);
    Module::external(ExternalModule::new(
      idx,
      ModuleId::new(id),
      id.into(),
      id.into(),
      DeterminedSideEffects::Analyzed(false),
      symbol_ref(idx),
      false,
    ))
  }

  pub(in crate::stages::link_stage) fn module_table(modules: Vec<Module>) -> ModuleTable {
    ModuleTable { modules: modules.into_iter().collect() }
  }

  pub(in crate::stages::link_stage) fn entry_point(
    index: usize,
    kind: EntryPointKind,
  ) -> EntryPoint {
    EntryPoint {
      name: None,
      idx: module_idx(index),
      kind,
      file_name: None,
      related_stmt_infos: Vec::new(),
    }
  }

  pub(in crate::stages::link_stage) fn reference_import_record_patches(
    module_count: usize,
    events: impl IntoIterator<Item = (ModuleIdx, rolldown_common::ImportRecordIdx)>,
  ) -> super::ReferenceImportRecordPatches {
    super::reference_needed_symbols::test_support::reference_import_record_patches(
      module_count,
      events,
    )
  }

  pub(in crate::stages::link_stage) fn cjs_routing_final(
    module_count: usize,
    routes: impl IntoIterator<Item = (ModuleIdx, SymbolRef, ModuleIdx)>,
  ) -> super::CjsRoutingFinal {
    super::compute_cjs_routing::test_support::cjs_routing_final(module_count, routes)
  }

  pub(in crate::stages::link_stage) fn statement_runtime_requirements(
    module_count: usize,
    requirements: impl IntoIterator<
      Item = (ModuleIdx, rolldown_common::RuntimeHelper, rolldown_common::StmtInfoIdx),
    >,
  ) -> super::StatementRuntimeRequirements {
    super::reference_needed_symbols::test_support::statement_runtime_requirements(
      module_count,
      requirements,
    )
  }
}
