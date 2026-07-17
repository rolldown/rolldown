use std::convert::Infallible;

use oxc_index::IndexVec;
use rolldown_common::{ModuleIdx, ModuleTable, RuntimeHelper, SymbolRefDb};
use rolldown_utils::{
  IndexBitSet,
  indexmap::FxIndexSet,
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
};

use crate::{
  stages::link_stage::finalize_module_dependencies_runner::finalize_module_dependencies,
  type_alias::IndexStmtInfos,
};

use super::{
  EntryExportRoots, FinalizeModuleDependenciesPass, InclusionResults, MemberExprResolutions,
  ModuleDependenciesDraft, ModuleRuntimeRequirementsDraft, ModuleSideEffects, RetainedEntries,
};

// See internal-docs/pass-based-pipeline/implementation.md.

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct FinalizeModuleDependenciesInput<'a> {
  pub module_table: &'a ModuleTable,
  pub stmt_infos: &'a IndexStmtInfos,
  pub symbols: &'a SymbolRefDb,
  pub inclusion: &'a InclusionResults,
  pub member_expr_resolutions: &'a MemberExprResolutions,
  pub module_side_effects: &'a ModuleSideEffects,
  pub retained_entries: &'a RetainedEntries,
  pub entry_export_roots: &'a EntryExportRoots,
  pub runtime_idx: ModuleIdx,
  pub tree_shaking_enabled: bool,
}

pub(in crate::stages::link_stage) struct FinalizeModuleDependenciesOwned {
  pub dependencies: ModuleDependenciesDraft,
  pub runtime_requirements: ModuleRuntimeRequirementsDraft,
}

pub(in crate::stages::link_stage) struct FinalizedModuleDependencies {
  dependencies: IndexVec<ModuleIdx, FxIndexSet<ModuleIdx>>,
  load_dependencies: IndexVec<ModuleIdx, FxIndexSet<ModuleIdx>>,
  runtime_requirements: IndexVec<ModuleIdx, RuntimeHelper>,
  side_effectful_runtime_dependencies: IndexBitSet<ModuleIdx>,
}

pub(in crate::stages::link_stage) type FinalizedModuleDependencyParts = (
  IndexVec<ModuleIdx, FxIndexSet<ModuleIdx>>,
  IndexVec<ModuleIdx, FxIndexSet<ModuleIdx>>,
  IndexVec<ModuleIdx, RuntimeHelper>,
  IndexBitSet<ModuleIdx>,
);

impl FinalizedModuleDependencies {
  pub(in crate::stages::link_stage) fn new(
    dependencies: IndexVec<ModuleIdx, FxIndexSet<ModuleIdx>>,
    load_dependencies: IndexVec<ModuleIdx, FxIndexSet<ModuleIdx>>,
    runtime_requirements: IndexVec<ModuleIdx, RuntimeHelper>,
    side_effectful_runtime_dependencies: IndexBitSet<ModuleIdx>,
  ) -> Self {
    Self {
      dependencies,
      load_dependencies,
      runtime_requirements,
      side_effectful_runtime_dependencies,
    }
  }

  pub(in crate::stages::link_stage) fn into_parts(self) -> FinalizedModuleDependencyParts {
    (
      self.dependencies,
      self.load_dependencies,
      self.runtime_requirements,
      self.side_effectful_runtime_dependencies,
    )
  }
}

impl Pass for FinalizeModuleDependenciesPass {
  type InputRead<'a> = FinalizeModuleDependenciesInput<'a>;
  type InputOwned = FinalizeModuleDependenciesOwned;
  type OutputRead = ();
  type OutputOwned = FinalizedModuleDependencies;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    Ok(token.finish((), finalize_module_dependencies(input, owned)))
  }
}

#[cfg(test)]
mod tests {
  use oxc::semantic::Scoping;
  use oxc_index::IndexVec;
  use rolldown_common::{
    EntryPointKind, MemberExprRefResolutionMap, ModuleNamespaceIncludedReason, RuntimeHelper,
    StmtInfo, StmtInfos, SymbolOrMemberExprRef, SymbolRefDb, SymbolRefDbForModule,
    side_effects::DeterminedSideEffects,
  };
  use rolldown_utils::{
    IndexBitSet,
    indexmap::{FxIndexMap, FxIndexSet},
    pass::{PassPipelineCtx, run_infallible_pass},
  };

  use super::super::{
    FinalizeModuleDependenciesInput, FinalizeModuleDependenciesOwned,
    FinalizeModuleDependenciesPass, InclusionResults, ModuleDependenciesDraft,
    ModuleRuntimeRequirementsDraft, RetainedEntries,
    collect_entry_export_roots::test_support::entry_export_roots,
    determine_module_side_effects::test_support::module_side_effects,
    resolve_member_expressions::test_support::member_expr_resolutions,
    test_utils::{entry_point, external_module, module_idx, module_table, normal_module},
  };
  use crate::type_alias::IndexStmtInfos;

  fn symbols_for(modules: &rolldown_common::ModuleTable) -> SymbolRefDb {
    let mut symbols = SymbolRefDb::new();
    for (module_idx, module) in modules.modules.iter_enumerated() {
      let scoping = Scoping::default();
      let root_scope_id = scoping.root_scope_id();
      symbols
        .store_local_db(module_idx, SymbolRefDbForModule::new(scoping, module_idx, root_scope_id));
      let expected = module.as_normal().map_or_else(
        || module.as_external().expect("external module").namespace_ref,
        |module| module.namespace_object_ref,
      );
      assert_eq!(symbols.create_facade_root_symbol_ref(module_idx, "namespace"), expected);
    }
    symbols
  }

  fn inclusion_for(
    stmt_infos: &IndexStmtInfos,
    included_statements: &[(usize, rolldown_common::StmtInfoIdx)],
    included_modules: &[usize],
  ) -> InclusionResults {
    let mut statement_bits = stmt_infos
      .iter()
      .map(|statements| IndexBitSet::new(statements.len()))
      .collect::<IndexVec<rolldown_common::ModuleIdx, _>>();
    for (module, statement) in included_statements {
      statement_bits[module_idx(*module)].set_bit(*statement);
    }
    let mut module_bits = IndexBitSet::new(stmt_infos.len());
    for module in included_modules {
      module_bits.set_bit(module_idx(*module));
    }
    InclusionResults::new(
      statement_bits,
      module_bits,
      oxc_index::index_vec![ModuleNamespaceIncludedReason::empty(); stmt_infos.len()],
    )
  }

  fn empty_statements(module_count: usize) -> IndexStmtInfos {
    (0..module_count).map(|_| StmtInfos::new()).collect()
  }

  fn dependency_slots(
    module_count: usize,
    entries: impl IntoIterator<Item = (usize, Vec<usize>)>,
  ) -> ModuleDependenciesDraft {
    let mut slots = (0..module_count)
      .map(|_| FxIndexSet::default())
      .collect::<IndexVec<rolldown_common::ModuleIdx, _>>();
    for (module, dependencies) in entries {
      slots[module_idx(module)].extend(dependencies.into_iter().map(module_idx));
    }
    ModuleDependenciesDraft::from_inner(slots)
  }

  fn runtime_slots(
    module_count: usize,
    entries: impl IntoIterator<Item = (usize, RuntimeHelper)>,
  ) -> ModuleRuntimeRequirementsDraft {
    let mut slots = oxc_index::index_vec![RuntimeHelper::default(); module_count];
    for (module, helpers) in entries {
      slots[module_idx(module)] = helpers;
    }
    ModuleRuntimeRequirementsDraft::new(slots)
  }

  #[test]
  fn inherits_to_esm_from_the_immutable_precommit_snapshot() {
    // Physical order is C(0), B(1), A(2), runtime(3). B inherits from C, but A must not observe
    // B's same-pass commit.
    let modules =
      module_table((0..4).map(|index| normal_module(index, false, Vec::new())).collect());
    let symbols = symbols_for(&modules);
    let statements = empty_statements(4);
    let inclusion = inclusion_for(&statements, &[], &[]);
    let resolutions =
      member_expr_resolutions((0..4).map(|_| Some(MemberExprRefResolutionMap::default())));
    let side_effects = module_side_effects(&[DeterminedSideEffects::Analyzed(false); 4]);
    let retained = RetainedEntries::new(FxIndexMap::default());
    let roots = entry_export_roots([]);
    let mut pipeline = PassPipelineCtx::new();
    let (_, finalized) = run_infallible_pass(
      FinalizeModuleDependenciesPass,
      &mut pipeline,
      FinalizeModuleDependenciesInput {
        module_table: &modules,
        stmt_infos: &statements,
        symbols: &symbols,
        inclusion: &inclusion,
        member_expr_resolutions: &resolutions,
        module_side_effects: &side_effects,
        retained_entries: &retained,
        entry_export_roots: &roots,
        runtime_idx: module_idx(3),
        tree_shaking_enabled: true,
      },
      FinalizeModuleDependenciesOwned {
        dependencies: dependency_slots(4, [(1, vec![0]), (2, vec![1])]),
        runtime_requirements: runtime_slots(4, [(0, RuntimeHelper::ToEsm)]),
      },
    );
    let (dependencies, load_dependencies, runtime_requirements, _) = finalized.into_parts();

    assert!(runtime_requirements[module_idx(1)].contains(RuntimeHelper::ToEsm));
    assert!(!runtime_requirements[module_idx(2)].contains(RuntimeHelper::ToEsm));
    assert_eq!(
      dependencies[module_idx(1)].iter().copied().collect::<Vec<_>>(),
      [module_idx(0), module_idx(3)]
    );
    assert_eq!(
      load_dependencies[module_idx(1)].iter().copied().collect::<Vec<_>>(),
      [module_idx(3)]
    );
    assert!(pipeline.into_diagnostics().is_empty());
  }

  #[test]
  fn preserves_final_and_load_dependency_order_for_both_tree_shaking_branches() {
    fn run(tree_shaking_enabled: bool) -> (Vec<usize>, Vec<usize>) {
      let modules =
        module_table((0..5).map(|index| normal_module(index, false, Vec::new())).collect());
      let symbols = symbols_for(&modules);
      let mut statements = empty_statements(5);
      let referenced =
        modules[module_idx(3)].as_normal().expect("normal module").namespace_object_ref;
      let statement = statements[module_idx(0)].add_stmt_info(
        StmtInfo::default()
          .with_referenced_symbols(vec![SymbolOrMemberExprRef::Symbol(referenced)]),
      );
      let inclusion = inclusion_for(&statements, &[(0, statement)], &[]);
      let resolutions =
        member_expr_resolutions((0..5).map(|_| Some(MemberExprRefResolutionMap::default())));
      let side_effects = module_side_effects(&[
        DeterminedSideEffects::Analyzed(false),
        DeterminedSideEffects::Analyzed(false),
        DeterminedSideEffects::Analyzed(true),
        DeterminedSideEffects::Analyzed(false),
        DeterminedSideEffects::Analyzed(false),
      ]);
      let retained = RetainedEntries::new(FxIndexMap::default());
      let roots = entry_export_roots([]);
      let mut pipeline = PassPipelineCtx::new();
      let (_, finalized) = run_infallible_pass(
        FinalizeModuleDependenciesPass,
        &mut pipeline,
        FinalizeModuleDependenciesInput {
          module_table: &modules,
          stmt_infos: &statements,
          symbols: &symbols,
          inclusion: &inclusion,
          member_expr_resolutions: &resolutions,
          module_side_effects: &side_effects,
          retained_entries: &retained,
          entry_export_roots: &roots,
          runtime_idx: module_idx(4),
          tree_shaking_enabled,
        },
        FinalizeModuleDependenciesOwned {
          dependencies: dependency_slots(5, [(0, vec![1, 2])]),
          runtime_requirements: runtime_slots(5, []),
        },
      );
      let (dependencies, load_dependencies, _, _) = finalized.into_parts();
      assert!(pipeline.into_diagnostics().is_empty());
      (
        dependencies[module_idx(0)].iter().map(|idx| idx.index()).collect(),
        load_dependencies[module_idx(0)].iter().map(|idx| idx.index()).collect(),
      )
    }

    assert_eq!(run(true), (vec![1, 2, 3], vec![3, 2]));
    assert_eq!(run(false), (vec![1, 2, 3], vec![3, 1, 2]));
  }

  #[test]
  fn injects_an_included_side_effectful_runtime_into_retained_entries() {
    let modules =
      module_table((0..2).map(|index| normal_module(index, false, Vec::new())).collect());
    let symbols = symbols_for(&modules);
    let statements = empty_statements(2);
    let inclusion = inclusion_for(&statements, &[], &[0]);
    let resolutions =
      member_expr_resolutions((0..2).map(|_| Some(MemberExprRefResolutionMap::default())));
    let side_effects = module_side_effects(&[
      DeterminedSideEffects::Analyzed(true),
      DeterminedSideEffects::Analyzed(false),
    ]);
    let retained = RetainedEntries::new(FxIndexMap::from_iter([(
      module_idx(1),
      vec![entry_point(1, EntryPointKind::UserDefined)],
    )]));
    let roots = entry_export_roots([]);
    let mut pipeline = PassPipelineCtx::new();
    let (_, finalized) = run_infallible_pass(
      FinalizeModuleDependenciesPass,
      &mut pipeline,
      FinalizeModuleDependenciesInput {
        module_table: &modules,
        stmt_infos: &statements,
        symbols: &symbols,
        inclusion: &inclusion,
        member_expr_resolutions: &resolutions,
        module_side_effects: &side_effects,
        retained_entries: &retained,
        entry_export_roots: &roots,
        runtime_idx: module_idx(0),
        tree_shaking_enabled: true,
      },
      FinalizeModuleDependenciesOwned {
        dependencies: dependency_slots(2, []),
        runtime_requirements: runtime_slots(2, []),
      },
    );
    let (dependencies, load_dependencies, _, side_effectful_runtime) = finalized.into_parts();

    assert_eq!(dependencies[module_idx(1)].iter().copied().collect::<Vec<_>>(), [module_idx(0)]);
    assert_eq!(
      load_dependencies[module_idx(1)].iter().copied().collect::<Vec<_>>(),
      [module_idx(0)]
    );
    assert!(side_effectful_runtime.has_bit(module_idx(1)));
    assert!(pipeline.into_diagnostics().is_empty());
  }

  #[test]
  fn external_modules_do_not_inherit_to_esm() {
    let modules = module_table(vec![
      normal_module(0, false, Vec::new()),
      external_module(1, "external"),
      normal_module(2, false, Vec::new()),
    ]);
    let symbols = symbols_for(&modules);
    let statements = empty_statements(3);
    let inclusion = inclusion_for(&statements, &[], &[]);
    let resolutions = member_expr_resolutions([
      Some(MemberExprRefResolutionMap::default()),
      None,
      Some(MemberExprRefResolutionMap::default()),
    ]);
    let side_effects = module_side_effects(&[DeterminedSideEffects::Analyzed(false); 3]);
    let retained = RetainedEntries::new(FxIndexMap::default());
    let roots = entry_export_roots([]);
    let mut pipeline = PassPipelineCtx::new();
    let (_, finalized) = run_infallible_pass(
      FinalizeModuleDependenciesPass,
      &mut pipeline,
      FinalizeModuleDependenciesInput {
        module_table: &modules,
        stmt_infos: &statements,
        symbols: &symbols,
        inclusion: &inclusion,
        member_expr_resolutions: &resolutions,
        module_side_effects: &side_effects,
        retained_entries: &retained,
        entry_export_roots: &roots,
        runtime_idx: module_idx(0),
        tree_shaking_enabled: true,
      },
      FinalizeModuleDependenciesOwned {
        dependencies: dependency_slots(3, [(1, vec![2])]),
        runtime_requirements: runtime_slots(3, [(2, RuntimeHelper::ToEsm)]),
      },
    );
    let (_, _, runtime_requirements, _) = finalized.into_parts();

    assert!(!runtime_requirements[module_idx(1)].contains(RuntimeHelper::ToEsm));
    assert!(pipeline.into_diagnostics().is_empty());
  }
}
