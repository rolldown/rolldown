use std::convert::Infallible;

use oxc_index::IndexVec;
use rolldown_common::{ImportKind, ModuleIdx, ModuleTable};
use rolldown_utils::{
  indexmap::FxIndexSet,
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
};

use super::CollectInitialDependenciesPass;

pub(in crate::stages::link_stage) struct ModuleDependenciesDraft {
  dependencies: IndexVec<ModuleIdx, FxIndexSet<ModuleIdx>>,
}

impl ModuleDependenciesDraft {
  pub(in crate::stages::link_stage) fn into_inner(
    self,
  ) -> IndexVec<ModuleIdx, FxIndexSet<ModuleIdx>> {
    self.dependencies
  }
}

impl Pass for CollectInitialDependenciesPass {
  type InputRead<'a> = &'a ModuleTable;
  type InputOwned = ();
  type OutputRead = ();
  type OutputOwned = ModuleDependenciesDraft;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    module_table: Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let dependencies = module_table
      .modules
      .iter()
      .map(|module| {
        module
          .import_records()
          .iter()
          .filter_map(|record| match record.kind {
            ImportKind::DynamicImport | ImportKind::Require => None,
            _ => record.resolved_module,
          })
          .collect()
      })
      .collect();

    Ok(token.finish((), ModuleDependenciesDraft { dependencies }))
  }
}

#[cfg(test)]
mod tests {
  use oxc::span::Span;
  use rolldown_common::ImportKind;
  use rolldown_utils::{
    indexmap::FxIndexSet,
    pass::{PassPipelineCtx, run_infallible_pass},
  };

  use super::super::test_utils::{module_idx, module_table, normal_module};
  use super::CollectInitialDependenciesPass;

  #[test]
  fn preserves_record_order_and_excludes_only_dynamic_and_require_edges() {
    let modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(1, 2)),
          (ImportKind::DynamicImport, Some(2), Span::new(2, 3)),
          (ImportKind::Require, Some(3), Span::new(3, 4)),
          (ImportKind::AtImport, Some(4), Span::new(4, 5)),
          (ImportKind::UrlImport, Some(5), Span::new(5, 6)),
          (ImportKind::NewUrl, Some(6), Span::new(6, 7)),
          (ImportKind::HotAccept, Some(7), Span::new(7, 8)),
          (ImportKind::Import, None, Span::new(8, 9)),
          (ImportKind::Import, Some(1), Span::new(9, 10)),
        ],
      ),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
      normal_module(3, false, Vec::new()),
      normal_module(4, false, Vec::new()),
      normal_module(5, false, Vec::new()),
      normal_module(6, false, Vec::new()),
      normal_module(7, false, Vec::new()),
    ]);
    let mut pipeline = PassPipelineCtx::new();
    let (_, dependencies) =
      run_infallible_pass(CollectInitialDependenciesPass, &mut pipeline, &modules, ());
    let dependencies = dependencies.into_inner();

    assert_eq!(
      dependencies[module_idx(0)].iter().copied().collect::<Vec<_>>(),
      [1, 4, 5, 6, 7].map(module_idx)
    );
    assert!(dependencies.iter().skip(1).all(FxIndexSet::is_empty));
    assert!(pipeline.into_diagnostics().is_empty());
  }
}
