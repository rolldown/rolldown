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
  use rolldown_common::MemberExprRefResolutionMap;

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
}
