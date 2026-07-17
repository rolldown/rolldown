use std::convert::Infallible;

use oxc_index::IndexVec;
use rolldown_common::{
  EntryPoint, ImportRecordIdx, ModuleIdx, ModuleTable, PropertyWriteSideEffects, RuntimeHelper,
  RuntimeModuleBrief, SymbolRef, SymbolRefDb, UsedExternalSymbols, UsedSymbolRefsBuilder,
};
use rolldown_utils::{
  IndexBitSet,
  indexmap::FxIndexMap,
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  stages::link_stage::{
    tree_shake_runner::run_tree_shake,
    tree_shaking::{ModuleInclusionVec, ModuleNamespaceReasonVec, StmtInclusionVec},
  },
  type_alias::IndexStmtInfos,
};

use super::{
  CjsRoutingFinal, EntryExportRoots, EntryPlanDraft, GlobalConstants,
  IncludedCommonJsExportSymbols, MemberExprResolutions, ModuleDependenciesDraft, ModuleFormats,
  ModuleSideEffects, ModuleWrappers, NormalExportChains, ResolvedExports,
  StatementRuntimeRequirements, TreeShakePass, UnreachableDynamicImports,
};

// See internal-docs/pass-based-pipeline/implementation.md.

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct TreeShakeOptions {
  pub inclusion: TreeShakeInclusionPolicy,
  pub preserve_modules: bool,
  pub dev_mode: bool,
  pub code_splitting_disabled: bool,
}

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct TreeShakeInclusionPolicy {
  pub tree_shaking_enabled: bool,
  pub commonjs_tree_shaking: bool,
  pub property_write_side_effects: PropertyWriteSideEffects,
  pub inline_const_smart: bool,
}

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct TreeShakeInput<'a> {
  pub module_table: &'a ModuleTable,
  pub stmt_infos: &'a IndexStmtInfos,
  pub symbols: &'a SymbolRefDb,
  pub runtime: &'a RuntimeModuleBrief,
  pub module_formats: &'a ModuleFormats,
  pub module_wrappers: &'a ModuleWrappers,
  pub resolved_exports: &'a ResolvedExports,
  pub cjs_routing: &'a CjsRoutingFinal,
  pub dependencies: &'a ModuleDependenciesDraft,
  pub member_expr_resolutions: &'a MemberExprResolutions,
  pub module_side_effects: &'a ModuleSideEffects,
  pub global_constants: &'a GlobalConstants,
  pub entry_export_roots: &'a EntryExportRoots,
  pub normal_export_chains: &'a NormalExportChains,
  pub included_commonjs_export_symbols: &'a IncludedCommonJsExportSymbols,
  pub dynamic_import_usage:
    &'a FxHashMap<ModuleIdx, rolldown_common::dynamic_import_usage::DynamicImportExportsUsage>,
  pub statement_runtime_requirements: &'a StatementRuntimeRequirements,
  pub unreachable_dynamic_imports: &'a UnreachableDynamicImports,
  pub options: TreeShakeOptions,
}

pub(in crate::stages::link_stage) struct TreeShakeOwned {
  pub entry_plan: EntryPlanDraft,
}

pub(in crate::stages::link_stage) struct RetainedEntries {
  entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
}

impl RetainedEntries {
  pub(in crate::stages::link_stage) fn new(
    entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
  ) -> Self {
    Self { entries }
  }

  pub(in crate::stages::link_stage) fn contains(&self, module_idx: ModuleIdx) -> bool {
    self.entries.contains_key(&module_idx)
  }

  pub(in crate::stages::link_stage) fn keys(&self) -> impl Iterator<Item = ModuleIdx> + '_ {
    self.entries.keys().copied()
  }

  pub(in crate::stages::link_stage) fn into_inner(self) -> FxIndexMap<ModuleIdx, Vec<EntryPoint>> {
    self.entries
  }
}

pub(in crate::stages::link_stage) struct InclusionResults {
  stmt_included: StmtInclusionVec,
  module_included: ModuleInclusionVec,
  namespace_reasons: ModuleNamespaceReasonVec,
}

impl InclusionResults {
  pub(in crate::stages::link_stage) fn new(
    stmt_included: StmtInclusionVec,
    module_included: ModuleInclusionVec,
    namespace_reasons: ModuleNamespaceReasonVec,
  ) -> Self {
    Self { stmt_included, module_included, namespace_reasons }
  }

  pub(in crate::stages::link_stage) fn module_count(&self) -> usize {
    self.stmt_included.len()
  }

  pub(in crate::stages::link_stage) fn stmt_included(
    &self,
    module_idx: ModuleIdx,
  ) -> &IndexBitSet<rolldown_common::StmtInfoIdx> {
    &self.stmt_included[module_idx]
  }

  pub(in crate::stages::link_stage) fn is_module_included(&self, module_idx: ModuleIdx) -> bool {
    self.module_included.has_bit(module_idx)
  }

  pub(in crate::stages::link_stage) fn into_parts(
    self,
  ) -> (StmtInclusionVec, ModuleInclusionVec, ModuleNamespaceReasonVec) {
    (self.stmt_included, self.module_included, self.namespace_reasons)
  }
}

pub(in crate::stages::link_stage) struct ModuleRuntimeRequirementsDraft {
  requirements: IndexVec<ModuleIdx, RuntimeHelper>,
}

impl ModuleRuntimeRequirementsDraft {
  pub(in crate::stages::link_stage) fn new(
    requirements: IndexVec<ModuleIdx, RuntimeHelper>,
  ) -> Self {
    Self { requirements }
  }

  pub(in crate::stages::link_stage) fn module_count(&self) -> usize {
    self.requirements.len()
  }

  pub(in crate::stages::link_stage) fn get(&self, module_idx: ModuleIdx) -> RuntimeHelper {
    self.requirements[module_idx]
  }

  pub(in crate::stages::link_stage) fn into_inner(self) -> IndexVec<ModuleIdx, RuntimeHelper> {
    self.requirements
  }
}

pub(in crate::stages::link_stage) struct TreeShakeModulePatches {
  json_non_self_references: JsonNonSelfReferences,
  dead_dynamic_imports: DeadDynamicImportPatches,
}

pub(in crate::stages::link_stage) type JsonNonSelfReferences =
  FxHashMap<ModuleIdx, FxHashSet<SymbolRef>>;
pub(in crate::stages::link_stage) type DeadDynamicImportPatches = Vec<(ModuleIdx, ImportRecordIdx)>;
pub(in crate::stages::link_stage) type TreeShakeModulePatchParts =
  (JsonNonSelfReferences, DeadDynamicImportPatches);

impl TreeShakeModulePatches {
  pub(in crate::stages::link_stage) fn new(
    json_non_self_references: JsonNonSelfReferences,
    dead_dynamic_imports: DeadDynamicImportPatches,
  ) -> Self {
    Self { json_non_self_references, dead_dynamic_imports }
  }

  pub(in crate::stages::link_stage) fn into_parts(self) -> TreeShakeModulePatchParts {
    (self.json_non_self_references, self.dead_dynamic_imports)
  }
}

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct EnumInliningPresence(bool);

impl EnumInliningPresence {
  pub(in crate::stages::link_stage) fn new(present: bool) -> Self {
    Self(present)
  }

  pub(in crate::stages::link_stage) fn get(self) -> bool {
    self.0
  }
}

pub(in crate::stages::link_stage) struct TreeShakeOutput {
  pub retained_entries: RetainedEntries,
  pub inclusion: InclusionResults,
  pub runtime_requirements: ModuleRuntimeRequirementsDraft,
  pub used_symbol_refs: UsedSymbolRefsBuilder,
  pub used_external_symbols: UsedExternalSymbols,
  pub module_patches: TreeShakeModulePatches,
  pub enum_inlining: EnumInliningPresence,
}

impl Pass for TreeShakePass {
  type InputRead<'a> = TreeShakeInput<'a>;
  type InputOwned = TreeShakeOwned;
  type OutputRead = ();
  type OutputOwned = TreeShakeOutput;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    Ok(token.finish((), run_tree_shake(input, owned.entry_plan)))
  }
}

#[cfg(test)]
mod tests {
  use oxc::{
    semantic::{Scoping, SymbolFlags},
    span::SPAN,
    syntax::{node::NodeId, scope::ScopeFlags},
  };
  use oxc_index::IndexVec;
  use oxc_str::Ident;
  use rolldown_common::{
    EntryPointKind, ExportsKind, MemberExprRefResolutionMap, ModuleNamespaceIncludedReason,
    ModuleType, PropertyWriteSideEffects, RUNTIME_MODULE_ID, RuntimeHelper, RuntimeModuleBrief,
    StmtEvalFlags, StmtInfo, StmtInfos, SymbolRef, SymbolRefDb, SymbolRefDbForModule,
    TaggedSymbolRef, WrapKind, side_effects::DeterminedSideEffects,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};
  use rustc_hash::FxHashMap;

  use super::super::{
    CanonicalizeEntriesPass, CollectInitialDependenciesPass, CollectResolvedExportsPass,
    FinalizeResolvedExportsPass,
    bind_imports::test_support::{empty_normal_export_chains, included_commonjs_export_symbols},
    collect_entry_export_roots::test_support::entry_export_roots,
    compute_cjs_routing::test_support::cjs_routing_final,
    create_wrapper_declarations::test_support::module_wrappers,
    cross_module_optimization::test_support::empty_unreachable_dynamic_imports,
    determine_module_formats::test_support::module_formats,
    determine_module_side_effects::test_support::module_side_effects,
    extract_global_constants::test_support::global_constants,
    resolve_member_expressions::test_support::member_expr_resolutions,
    test_utils::{entry_point, module_idx, module_table, normal_module, normal_module_with_id},
  };
  use super::*;

  #[test]
  fn tree_shake_output_artifacts_have_independent_types() {
    let _: Option<RetainedEntries> = None;
    let _: Option<InclusionResults> = None;
    let _: Option<ModuleRuntimeRequirementsDraft> = None;
    let _: Option<TreeShakeModulePatches> = None;
    let _: Option<EnumInliningPresence> = None;
    let _: Option<IndexVec<ModuleIdx, Option<MemberExprRefResolutionMap>>> = None;
  }

  #[test]
  fn keeps_phase_one_projection_while_discarding_phase_two_scratch() {
    let runtime_idx = module_idx(0);
    let entry_idx = module_idx(1);
    let phase_one_json_idx = module_idx(2);
    let phase_two_json_idx = module_idx(3);
    let mut modules = module_table(vec![
      normal_module_with_id(0, &RUNTIME_MODULE_ID, false, Vec::new()),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
      normal_module(3, false, Vec::new()),
    ]);
    modules[phase_one_json_idx].as_normal_mut().expect("phase-one JSON module").module_type =
      ModuleType::Json;
    modules[phase_two_json_idx].as_normal_mut().expect("phase-two JSON module").module_type =
      ModuleType::Json;

    let mut symbols = SymbolRefDb::new();
    let mut runtime_helper_ref = None;
    for (module_idx, module) in modules.modules.iter_enumerated() {
      let mut scoping = Scoping::default();
      scoping.add_scope(None, NodeId::DUMMY, ScopeFlags::Top);
      let root_scope_id = scoping.root_scope_id();
      if module_idx == runtime_idx {
        let namespace_symbol = scoping.create_symbol(
          SPAN,
          Ident::from("namespace"),
          SymbolFlags::empty(),
          root_scope_id,
          NodeId::DUMMY,
        );
        scoping.add_binding(root_scope_id, Ident::from("namespace"), namespace_symbol);
        let helper_symbol = scoping.create_symbol(
          SPAN,
          Ident::from("__toESM"),
          SymbolFlags::empty(),
          root_scope_id,
          NodeId::DUMMY,
        );
        scoping.add_binding(root_scope_id, Ident::from("__toESM"), helper_symbol);
        symbols.store_local_db(
          module_idx,
          SymbolRefDbForModule::new(scoping, module_idx, root_scope_id),
        );
        let namespace_ref: SymbolRef = (module_idx, namespace_symbol).into();
        assert_eq!(
          namespace_ref,
          module.as_normal().expect("normal fixture module").namespace_object_ref
        );
        runtime_helper_ref = Some((module_idx, helper_symbol).into());
      } else {
        symbols.store_local_db(
          module_idx,
          SymbolRefDbForModule::new(scoping, module_idx, root_scope_id),
        );
        assert_eq!(
          symbols.create_facade_root_symbol_ref(module_idx, "namespace"),
          module.as_normal().expect("normal fixture module").namespace_object_ref
        );
      }
    }
    let runtime_helper_ref = runtime_helper_ref.expect("runtime helper symbol");
    let phase_one_json_namespace =
      modules[phase_one_json_idx].as_normal().expect("phase-one JSON module").namespace_object_ref;
    let phase_two_json_namespace =
      modules[phase_two_json_idx].as_normal().expect("phase-two JSON module").namespace_object_ref;
    let runtime = RuntimeModuleBrief::new(
      runtime_idx,
      &symbols[runtime_idx].as_ref().expect("runtime symbols").ast_scopes,
    );

    let mut stmt_infos =
      (0..modules.modules.len()).map(|_| StmtInfos::new()).collect::<IndexVec<ModuleIdx, _>>();
    for (module_idx, namespace_ref) in [
      (phase_one_json_idx, phase_one_json_namespace),
      (phase_two_json_idx, phase_two_json_namespace),
    ] {
      let mut namespace_stmt = StmtInfo::default();
      namespace_stmt.declared_symbols.push(TaggedSymbolRef::normal(namespace_ref));
      stmt_infos[module_idx].replace_namespace_stmt_info(namespace_stmt);
    }
    let mut entry_stmt =
      StmtInfo::default().with_referenced_symbols(vec![phase_one_json_namespace.into()]);
    entry_stmt.eval_flags = StmtEvalFlags::UnknownSideEffect;
    let entry_stmt_idx = stmt_infos[entry_idx].add_stmt_info(entry_stmt);
    let mut runtime_stmt =
      StmtInfo::default().with_referenced_symbols(vec![phase_two_json_namespace.into()]);
    runtime_stmt.declared_symbols.push(TaggedSymbolRef::normal(runtime_helper_ref));
    let runtime_stmt_idx = stmt_infos[runtime_idx].add_stmt_info(runtime_stmt);

    let mut pipeline = PassPipelineCtx::new();
    let (_, entry_plan) = run_infallible_pass(
      CanonicalizeEntriesPass,
      &mut pipeline,
      &modules,
      vec![entry_point(entry_idx.index(), EntryPointKind::UserDefined)],
    );
    let (_, dependencies) =
      run_infallible_pass(CollectInitialDependenciesPass, &mut pipeline, &modules, ());
    let (_, resolved_exports_draft) =
      run_infallible_pass(CollectResolvedExportsPass, &mut pipeline, &modules, ());
    let (_, resolved_exports) = run_infallible_pass(
      FinalizeResolvedExportsPass,
      &mut pipeline,
      &symbols,
      resolved_exports_draft,
    );
    assert!(pipeline.into_diagnostics().is_empty());

    let module_count = modules.modules.len();
    let module_formats = module_formats(&vec![Some(ExportsKind::Esm); module_count]);
    let module_wrappers = module_wrappers(&vec![WrapKind::None; module_count]);
    let cjs_routing = cjs_routing_final(module_count, []);
    let member_expr_resolutions = member_expr_resolutions(
      (0..module_count).map(|_| Some(MemberExprRefResolutionMap::default())),
    );
    let module_side_effects =
      module_side_effects(&vec![DeterminedSideEffects::Analyzed(false); module_count]);
    let global_constants = global_constants([]).finalize();
    let entry_export_roots = entry_export_roots([]);
    let normal_export_chains = empty_normal_export_chains();
    let included_commonjs_export_symbols =
      included_commonjs_export_symbols((0..module_count).map(|_| Some(Vec::new())));
    let dynamic_import_usage = FxHashMap::default();
    let statement_runtime_requirements =
      super::super::reference_needed_symbols::test_support::statement_runtime_requirements(
        module_count,
        [(entry_idx, RuntimeHelper::ToEsm, entry_stmt_idx)],
      );
    let unreachable_dynamic_imports = empty_unreachable_dynamic_imports();

    let output = run_tree_shake(
      TreeShakeInput {
        module_table: &modules,
        stmt_infos: &stmt_infos,
        symbols: &symbols,
        runtime: &runtime,
        module_formats: &module_formats,
        module_wrappers: &module_wrappers,
        resolved_exports: &resolved_exports,
        cjs_routing: &cjs_routing,
        dependencies: &dependencies,
        member_expr_resolutions: &member_expr_resolutions,
        module_side_effects: &module_side_effects,
        global_constants: &global_constants,
        entry_export_roots: &entry_export_roots,
        normal_export_chains: &normal_export_chains,
        included_commonjs_export_symbols: &included_commonjs_export_symbols,
        dynamic_import_usage: &dynamic_import_usage,
        statement_runtime_requirements: &statement_runtime_requirements,
        unreachable_dynamic_imports: &unreachable_dynamic_imports,
        options: TreeShakeOptions {
          inclusion: TreeShakeInclusionPolicy {
            tree_shaking_enabled: true,
            commonjs_tree_shaking: false,
            property_write_side_effects: PropertyWriteSideEffects::False,
            inline_const_smart: false,
          },
          preserve_modules: false,
          dev_mode: false,
          code_splitting_disabled: false,
        },
      },
      entry_plan,
    );

    assert!(output.runtime_requirements.get(entry_idx).contains(RuntimeHelper::ToEsm));
    let (stmt_included, _, namespace_reasons) = output.inclusion.into_parts();
    assert!(stmt_included[entry_idx].has_bit(entry_stmt_idx));
    assert!(stmt_included[runtime_idx].has_bit(runtime_stmt_idx));
    assert!(stmt_included[phase_one_json_idx].has_bit(StmtInfos::NAMESPACE_STMT_IDX));
    assert!(stmt_included[phase_two_json_idx].has_bit(StmtInfos::NAMESPACE_STMT_IDX));
    assert_eq!(namespace_reasons[phase_one_json_idx], ModuleNamespaceIncludedReason::Unknown);
    assert!(namespace_reasons[phase_two_json_idx].is_empty());

    let (json_non_self_references, dead_dynamic_imports) = output.module_patches.into_parts();
    assert_eq!(json_non_self_references.len(), 1);
    let phase_one_json_references =
      json_non_self_references.get(&phase_one_json_idx).expect("phase-one JSON references");
    assert_eq!(phase_one_json_references.len(), 1);
    assert!(phase_one_json_references.contains(&phase_one_json_namespace));
    assert!(!json_non_self_references.contains_key(&phase_two_json_idx));
    assert!(dead_dynamic_imports.is_empty());
  }
}
