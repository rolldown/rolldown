//! Typed link passes and their narrow artifacts.

#![forbid(unsafe_code)]

mod canonicalize_entries;
mod collect_external_star_exports;
mod collect_initial_dependencies;
mod compute_cjs_namespace_merges;
mod compute_dynamic_exports;
mod compute_module_execution_order;
mod compute_tla;
mod create_wrapper_declarations;
mod determine_module_formats;
mod determine_module_side_effects;
mod extract_global_constants;
mod normalize_lazy_exports;
mod plan_module_wrapping;

pub(super) use canonicalize_entries::EntryPlanDraft;
pub(super) use compute_cjs_namespace_merges::{CjsNamespaceMerges, ComputeCjsNamespaceMergesInput};
pub(super) use compute_dynamic_exports::{ComputeDynamicExportsInput, DynamicExports};
pub(super) use compute_module_execution_order::ComputeModuleExecutionOrderInput;
pub(super) use compute_tla::TlaScanFacts;
pub(super) use create_wrapper_declarations::{
  CreateWrapperDeclarationsInput, CreateWrapperDeclarationsOutput, CreateWrapperDeclarationsOwned,
  ModuleWrappers, WrapperDeclaration, WrapperDeclarationsDraft,
};
pub(super) use determine_module_formats::{
  DetermineModuleFormatsInput, ModuleFormats, ModuleFormatsDraft,
};
pub(super) use determine_module_side_effects::{
  DetermineModuleSideEffectsInput, ModuleSideEffects,
};
pub(super) use extract_global_constants::{ConstantExtractionInput, GlobalConstantsDraft};
pub(super) use normalize_lazy_exports::{
  NormalizeLazyExportsInput, NormalizeLazyExportsOutput, NormalizeLazyExportsOwned,
};
pub(super) use plan_module_wrapping::PlanModuleWrappingInput;

#[derive(Clone, Copy)]
pub(super) struct CanonicalizeEntriesPass;

#[derive(Clone, Copy)]
pub(super) struct CollectExternalStarExportsPass;

#[derive(Clone, Copy)]
pub(super) struct CollectInitialDependenciesPass;

#[derive(Clone, Copy)]
pub(super) struct ComputeCjsNamespaceMergesPass;

#[derive(Clone, Copy)]
pub(super) struct ComputeDynamicExportsPass;

#[derive(Clone, Copy)]
pub(super) struct ComputeModuleExecutionOrderPass;

#[derive(Clone, Copy)]
pub(super) struct ComputeTlaPass;

#[derive(Clone, Copy)]
pub(super) struct CreateWrapperDeclarationsPass;

#[derive(Clone, Copy)]
pub(super) struct DetermineModuleFormatsPass;

#[derive(Clone, Copy)]
pub(super) struct DetermineModuleSideEffectsPass;

#[derive(Clone, Copy)]
pub(super) struct ExtractGlobalConstantsPass;

#[derive(Clone, Copy)]
pub(super) struct NormalizeLazyExportsPass;

#[derive(Clone, Copy)]
pub(super) struct PlanModuleWrappingPass;

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

  pub(super) fn normal_module_with_id(
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

  pub(super) fn external_module(index: usize, id: &str) -> Module {
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

  pub(super) fn module_table(modules: Vec<Module>) -> ModuleTable {
    ModuleTable { modules: modules.into_iter().collect() }
  }

  pub(super) fn entry_point(index: usize, kind: EntryPointKind) -> EntryPoint {
    EntryPoint {
      name: None,
      idx: module_idx(index),
      kind,
      file_name: None,
      related_stmt_infos: Vec::new(),
    }
  }
}
