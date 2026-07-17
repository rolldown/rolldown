use rolldown_common::{ModuleIdx, RuntimeHelper, SymbolOrMemberExprRef};
use rolldown_utils::{
  IndexBitSet, index_vec_ext::IndexVecRefExt, indexmap::FxIndexSet, rayon::ParallelIterator,
};

use crate::stages::link_stage::passes::{
  FinalizeModuleDependenciesInput, FinalizeModuleDependenciesOwned, FinalizedModuleDependencies,
};

struct ModuleDependencyAnalysis {
  extended_dependencies: FxIndexSet<ModuleIdx>,
  inherited_runtime: RuntimeHelper,
}

fn assert_finalize_layout(
  input: FinalizeModuleDependenciesInput<'_>,
  owned: &FinalizeModuleDependenciesOwned,
) {
  let module_count = input.module_table.modules.len();
  for (domain, actual) in [
    ("statement", input.stmt_infos.len()),
    ("symbol", input.symbols.inner().len()),
    ("inclusion", input.inclusion.module_count()),
    ("member-resolution", input.member_expr_resolutions.module_count()),
    ("side-effect", input.module_side_effects.module_count()),
    ("dependency", owned.dependencies.module_count()),
    ("runtime-requirement", owned.runtime_requirements.module_count()),
  ] {
    assert_eq!(
      actual, module_count,
      "{domain} layout must match modules before dependency finalization"
    );
  }
  assert!(
    input
      .module_table
      .modules
      .get(input.runtime_idx)
      .is_some_and(|module| module.as_normal().is_some()),
    "runtime must be an in-range normal module before dependency finalization"
  );
}

fn analyze_module(
  module_idx: ModuleIdx,
  input: FinalizeModuleDependenciesInput<'_>,
  owned: &FinalizeModuleDependenciesOwned,
) -> ModuleDependencyAnalysis {
  let mut extended_dependencies = FxIndexSet::default();
  if !owned.runtime_requirements.get(module_idx).is_empty() {
    extended_dependencies.insert(input.runtime_idx);
  }
  for &(symbol_ref, _) in input.entry_export_roots.get(module_idx).unwrap_or_default() {
    let canonical_ref = input.symbols.canonical_ref_for(symbol_ref);
    extended_dependencies.insert(canonical_ref.owner);
    if let Some(alias) = &input.symbols.get(canonical_ref).namespace_alias {
      extended_dependencies.insert(alias.namespace_ref.owner);
    }
  }

  let inherited_runtime = if input.module_table[module_idx].as_normal().is_some() {
    let resolutions = input
      .member_expr_resolutions
      .get(module_idx)
      .expect("normal module must have a member-resolution slot");
    for (_, stmt_info) in input.stmt_infos[module_idx]
      .iter_enumerated()
      .filter(|(stmt_idx, _)| input.inclusion.stmt_included(module_idx).has_bit(*stmt_idx))
    {
      for reference in &stmt_info.referenced_symbols {
        let represented = match reference {
          SymbolOrMemberExprRef::Symbol(symbol_ref) => Some(*symbol_ref),
          SymbolOrMemberExprRef::MemberExpr(member_expr) => {
            member_expr.represent_symbol_ref(resolutions)
          }
        };
        let Some(symbol_ref) = represented else {
          continue;
        };
        let canonical_ref = input.symbols.canonical_ref_for(symbol_ref);
        extended_dependencies.insert(canonical_ref.owner);
        if let Some(alias) = &input.symbols.get(canonical_ref).namespace_alias {
          extended_dependencies.insert(alias.namespace_ref.owner);
        }
      }
    }
    if owned.dependencies.iter(module_idx).any(|dependency_idx| {
      input.module_table[dependency_idx].as_normal().is_some()
        && !input.inclusion.is_module_included(dependency_idx)
        && owned.runtime_requirements.get(dependency_idx).contains(RuntimeHelper::ToEsm)
    }) {
      RuntimeHelper::ToEsm
    } else {
      RuntimeHelper::default()
    }
  } else {
    RuntimeHelper::default()
  };
  ModuleDependencyAnalysis { extended_dependencies, inherited_runtime }
}

pub(in crate::stages::link_stage) fn finalize_module_dependencies(
  input: FinalizeModuleDependenciesInput<'_>,
  owned: FinalizeModuleDependenciesOwned,
) -> FinalizedModuleDependencies {
  assert_finalize_layout(input, &owned);
  let module_count = input.module_table.modules.len();
  let analyzed = input
    .module_table
    .modules
    .par_iter_enumerated()
    .map(|(module_idx, _)| analyze_module(module_idx, input, &owned))
    .collect::<Vec<_>>();

  let mut dependencies = owned.dependencies.into_inner();
  let mut runtime_requirements = owned.runtime_requirements.into_inner();
  let mut load_dependencies = dependencies
    .iter()
    .map(|_| FxIndexSet::default())
    .collect::<oxc_index::IndexVec<ModuleIdx, _>>();
  let mut side_effectful_runtime_dependencies = IndexBitSet::new(module_count);

  for (module_idx, result) in analyzed.into_iter().enumerate() {
    let module_idx = ModuleIdx::from_usize(module_idx);
    load_dependencies[module_idx].extend(result.extended_dependencies.iter().copied());
    load_dependencies[module_idx].extend(dependencies[module_idx].iter().copied().filter(
      |dependency_idx| {
        !input.tree_shaking_enabled
          || input.retained_entries.contains(*dependency_idx)
          || input.module_side_effects.get(*dependency_idx).has_side_effects()
      },
    ));
    dependencies[module_idx].extend(result.extended_dependencies);
    runtime_requirements[module_idx] |= result.inherited_runtime;
    if !result.inherited_runtime.is_empty() {
      dependencies[module_idx].insert(input.runtime_idx);
      load_dependencies[module_idx].insert(input.runtime_idx);
    }
  }

  let runtime_idx = input.runtime_idx;
  if input.inclusion.is_module_included(runtime_idx)
    && input.module_side_effects.get(runtime_idx).has_side_effects()
  {
    for entry_idx in input.retained_entries.keys() {
      dependencies[entry_idx].insert(runtime_idx);
      load_dependencies[entry_idx].insert(runtime_idx);
      side_effectful_runtime_dependencies.set_bit(entry_idx);
    }
  }

  FinalizedModuleDependencies::new(
    dependencies,
    load_dependencies,
    runtime_requirements,
    side_effectful_runtime_dependencies,
  )
}
