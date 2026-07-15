use std::convert::Infallible;

use rolldown_common::{EntryPoint, EntryPointKind, ModuleIdx, ModuleTable};
use rolldown_utils::{
  indexmap::FxIndexMap,
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
};

use super::CanonicalizeEntriesPass;

pub(in crate::stages::link_stage) struct EntryPlanDraft {
  entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
}

impl EntryPlanDraft {
  pub(in crate::stages::link_stage) fn contains_root(&self, module_idx: ModuleIdx) -> bool {
    self.entries.contains_key(&module_idx)
  }

  pub(in crate::stages::link_stage) fn roots(
    &self,
  ) -> impl DoubleEndedIterator<Item = ModuleIdx> + '_ {
    self.entries.keys().copied()
  }

  pub(in crate::stages::link_stage) fn into_legacy_entries(
    self,
  ) -> FxIndexMap<ModuleIdx, Vec<EntryPoint>> {
    self.entries
  }
}

impl Pass for CanonicalizeEntriesPass {
  type InputRead<'a> = &'a ModuleTable;
  type InputOwned = Vec<EntryPoint>;
  type OutputRead = ();
  type OutputOwned = EntryPlanDraft;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    module_table: Self::InputRead<'_>,
    mut entry_points: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    // Preserve the original order of user-defined entries. Dynamic and emitted
    // entries arrive from concurrent discovery and need a stable canonical order.
    let mut rest = entry_points
      .extract_if(0.., |item| !std::matches!(item.kind, EntryPointKind::UserDefined))
      .collect::<Vec<_>>();
    rest.sort_by_cached_key(|item| (item.kind, module_table[item.idx].id().as_str()));
    entry_points.extend(rest);

    let mut entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>> = FxIndexMap::default();
    for entry in entry_points {
      entries.entry(entry.idx).or_default().push(entry);
    }
    Ok(token.finish((), EntryPlanDraft { entries }))
  }
}

#[cfg(test)]
mod tests {
  use rolldown_common::{EntryPointKind, ModuleIdx};
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::test_utils::{entry_point, module_idx, module_table, normal_module_with_id};
  use super::CanonicalizeEntriesPass;

  #[test]
  fn preserves_user_order_and_canonicalizes_discovered_entries() {
    let modules = module_table(vec![
      normal_module_with_id(0, "runtime.js", false, Vec::new()),
      normal_module_with_id(1, "user-z.js", false, Vec::new()),
      normal_module_with_id(2, "user-a.js", false, Vec::new()),
      normal_module_with_id(3, "dynamic-z.js", false, Vec::new()),
      normal_module_with_id(4, "dynamic-a.js", false, Vec::new()),
      normal_module_with_id(5, "emitted-z.js", false, Vec::new()),
      normal_module_with_id(6, "emitted-a.js", false, Vec::new()),
    ]);
    let input = vec![
      entry_point(1, EntryPointKind::UserDefined),
      entry_point(2, EntryPointKind::UserDefined),
      entry_point(3, EntryPointKind::DynamicImport),
      entry_point(5, EntryPointKind::EmittedUserDefined),
      entry_point(4, EntryPointKind::DynamicImport),
      entry_point(6, EntryPointKind::EmittedUserDefined),
      entry_point(4, EntryPointKind::DynamicImport),
    ];

    let mut pipeline = PassPipelineCtx::new();
    let (_, entry_plan) =
      run_infallible_pass(CanonicalizeEntriesPass, &mut pipeline, &modules, input);

    let expected = [1, 2, 4, 3, 6, 5].map(module_idx);
    assert_eq!(entry_plan.roots().collect::<Vec<_>>(), expected);
    let entries = entry_plan.into_legacy_entries();
    assert_eq!(entries.keys().copied().collect::<Vec<ModuleIdx>>(), expected);
    assert_eq!(entries.get(&module_idx(4)).expect("grouped dynamic entry").len(), 2);
    assert!(pipeline.into_diagnostics().is_empty());
  }
}
