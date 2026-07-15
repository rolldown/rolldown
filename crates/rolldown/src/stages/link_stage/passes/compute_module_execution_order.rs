use std::{convert::Infallible, iter};

use oxc_index::IndexVec;
use rolldown_common::{ModuleIdx, ModuleTable};
use rolldown_error::BuildDiagnostic;
use rolldown_utils::{
  indexmap::FxIndexSet,
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
  rustc_hash::FxHashSetExt,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{ComputeModuleExecutionOrderPass, EntryPlanDraft};

enum Status {
  ToBeExecuted(ModuleIdx),
  WaitForExit(ModuleIdx),
}

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct ComputeModuleExecutionOrderInput<'a> {
  pub module_table: &'a ModuleTable,
  pub entry_plan: &'a EntryPlanDraft,
  pub runtime: ModuleIdx,
  pub code_splitting_disabled: bool,
  pub check_circular_dependencies: bool,
}

#[derive(Debug)]
pub(in crate::stages::link_stage) struct ModuleExecutionOrders {
  orders: IndexVec<ModuleIdx, u32>,
}

impl ModuleExecutionOrders {
  pub(in crate::stages::link_stage) fn get(&self, module_idx: ModuleIdx) -> u32 {
    self.orders[module_idx]
  }

  pub(in crate::stages::link_stage) fn assigned(
    &self,
  ) -> impl Iterator<Item = (ModuleIdx, u32)> + '_ {
    self
      .orders
      .iter_enumerated()
      .filter_map(|(idx, order)| (*order != u32::MAX).then_some((idx, *order)))
  }
}

pub(in crate::stages::link_stage) struct SortedModules {
  modules: Vec<ModuleIdx>,
}

impl SortedModules {
  pub(in crate::stages::link_stage) fn into_inner(self) -> Vec<ModuleIdx> {
    self.modules
  }
}

impl Pass for ComputeModuleExecutionOrderPass {
  type InputRead<'a> = ComputeModuleExecutionOrderInput<'a>;
  type InputOwned = ();
  type OutputRead = ModuleExecutionOrders;
  type OutputOwned = SortedModules;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let ComputeModuleExecutionOrderInput {
      module_table,
      entry_plan,
      runtime,
      code_splitting_disabled,
      check_circular_dependencies,
    } = input;
    let mut execution_stack = entry_plan
      .roots()
      .rev()
      .map(Status::ToBeExecuted)
      .chain(iter::once(Status::ToBeExecuted(runtime)))
      .collect::<Vec<_>>();

    let mut next_exec_order = 0;
    let mut execution_orders = oxc_index::index_vec![u32::MAX; module_table.modules.len()];
    let mut executed_ids = FxHashSet::with_capacity(module_table.modules.len());
    let mut stack_indexes_of_executing_id = FxHashMap::default();
    let mut sorted_modules = Vec::with_capacity(module_table.modules.len());
    // Keep the first traversal discovery order. `FxHashSet` made independent
    // cycle diagnostics change order across otherwise identical processes.
    let mut circular_dependencies = FxIndexSet::default();

    while let Some(status) = execution_stack.pop() {
      match status {
        Status::ToBeExecuted(id) => {
          if executed_ids.contains(&id) {
            if check_circular_dependencies
              && let Some(index) = stack_indexes_of_executing_id.get(&id).copied()
            {
              let cycle = execution_stack[index..]
                .iter()
                .filter_map(|action| match action {
                  Status::ToBeExecuted(_) => None,
                  Status::WaitForExit(id) => Some(*id),
                })
                .chain(iter::once(id))
                .collect::<Box<[_]>>();
              circular_dependencies.insert(cycle);
            }
          } else {
            executed_ids.insert(id);
            execution_stack.push(Status::WaitForExit(id));
            std::debug_assert!(
              !stack_indexes_of_executing_id.contains_key(&id),
              "a module should not be executing the same module twice"
            );
            stack_indexes_of_executing_id.insert(id, execution_stack.len() - 1);
            execution_stack.extend(
              module_table[id]
                .import_records()
                .iter()
                .filter(|record| {
                  record.kind.is_static() || (code_splitting_disabled && record.kind.is_dynamic())
                })
                .filter_map(|record| record.resolved_module)
                .rev()
                .map(Status::ToBeExecuted),
            );
          }
        }
        Status::WaitForExit(id) => {
          std::debug_assert!(
            module_table[id].exec_order() == u32::MAX && execution_orders[id] == u32::MAX,
            "execution order must be assigned exactly once"
          );
          execution_orders[id] = next_exec_order;
          if module_table[id].as_normal().is_some() {
            sorted_modules.push(id);
          }
          next_exec_order += 1;
          std::debug_assert!(
            stack_indexes_of_executing_id.contains_key(&id),
            "the exiting module must still be on the execution stack"
          );
          stack_indexes_of_executing_id.remove(&id);
        }
      }
    }

    for cycle in circular_dependencies {
      let paths = cycle
        .iter()
        .filter_map(|id| module_table[*id].as_normal().map(|module| module.id.to_string()))
        .collect::<Vec<_>>();
      cx.push(BuildDiagnostic::circular_dependency(paths).with_severity_warning());
    }

    std::debug_assert!(
      sorted_modules.first().copied() == Some(runtime),
      "runtime module should always be first in the sorted modules"
    );
    Ok(token.finish(
      ModuleExecutionOrders { orders: execution_orders },
      SortedModules { modules: sorted_modules },
    ))
  }
}

#[cfg(test)]
mod tests {
  use oxc::span::Span;
  use rolldown_common::{EntryPointKind, ImportKind, ModuleTable};
  use rolldown_utils::pass::{PassPipelineCtx, Sealed, run_infallible_pass};

  use super::super::{
    CanonicalizeEntriesPass,
    test_utils::{entry_point, external_module, module_idx, module_table, normal_module},
  };
  use super::{
    ComputeModuleExecutionOrderInput, ComputeModuleExecutionOrderPass, EntryPlanDraft,
    ModuleExecutionOrders,
  };

  fn assert_sealed(_: &Sealed<ModuleExecutionOrders>) {}

  fn entry_plan(modules: &ModuleTable, entries: &[usize]) -> EntryPlanDraft {
    let mut pipeline = PassPipelineCtx::new();
    let (_, entry_plan) = run_infallible_pass(
      CanonicalizeEntriesPass,
      &mut pipeline,
      modules,
      entries
        .iter()
        .copied()
        .map(|index| entry_point(index, EntryPointKind::UserDefined))
        .collect(),
    );
    assert!(pipeline.into_diagnostics().is_empty());
    entry_plan
  }

  #[test]
  fn computes_orders_and_emits_independent_cycles_in_first_discovery_order() {
    let modules = module_table(vec![
      normal_module(0, false, Vec::new()),
      normal_module(1, false, vec![(ImportKind::Import, Some(2), Span::new(10, 11))]),
      normal_module(2, false, vec![(ImportKind::Import, Some(1), Span::new(20, 21))]),
      normal_module(3, false, vec![(ImportKind::Import, Some(4), Span::new(30, 31))]),
      normal_module(4, false, vec![(ImportKind::Import, Some(3), Span::new(40, 41))]),
    ]);
    let entry_plan = entry_plan(&modules, &[1, 3]);
    let mut pipeline = PassPipelineCtx::new();
    let (orders, sorted) = run_infallible_pass(
      ComputeModuleExecutionOrderPass,
      &mut pipeline,
      ComputeModuleExecutionOrderInput {
        module_table: &modules,
        entry_plan: &entry_plan,
        runtime: module_idx(0),
        code_splitting_disabled: false,
        check_circular_dependencies: true,
      },
      (),
    );

    assert_eq!(
      orders.orders.iter_enumerated().map(|(idx, order)| (idx, *order)).collect::<Vec<_>>(),
      vec![
        (module_idx(0), 0),
        (module_idx(1), 2),
        (module_idx(2), 1),
        (module_idx(3), 4),
        (module_idx(4), 3),
      ]
    );
    assert_eq!(
      sorted.into_inner(),
      vec![module_idx(0), module_idx(2), module_idx(1), module_idx(4), module_idx(3)]
    );
    let diagnostics = pipeline.into_diagnostics().into_iter().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].to_string().contains("m1.js -> m2.js -> m1.js"));
    assert!(diagnostics[1].to_string().contains("m3.js -> m4.js -> m3.js"));
  }

  #[test]
  fn follows_dynamic_imports_only_when_code_splitting_is_disabled() {
    let modules = module_table(vec![
      normal_module(0, false, Vec::new()),
      normal_module(1, false, vec![(ImportKind::DynamicImport, Some(2), Span::new(10, 11))]),
      normal_module(2, false, Vec::new()),
    ]);
    let entry_plan = entry_plan(&modules, &[1]);

    for (code_splitting_disabled, expected) in [
      (false, vec![(module_idx(0), 0), (module_idx(1), 1), (module_idx(2), u32::MAX)]),
      (true, vec![(module_idx(0), 0), (module_idx(1), 2), (module_idx(2), 1)]),
    ] {
      let mut pipeline = PassPipelineCtx::new();
      let (orders, _) = run_infallible_pass(
        ComputeModuleExecutionOrderPass,
        &mut pipeline,
        ComputeModuleExecutionOrderInput {
          module_table: &modules,
          entry_plan: &entry_plan,
          runtime: module_idx(0),
          code_splitting_disabled,
          check_circular_dependencies: false,
        },
        (),
      );
      assert_eq!(
        orders.orders.iter_enumerated().map(|(idx, order)| (idx, *order)).collect::<Vec<_>>(),
        expected
      );
      assert!(pipeline.into_diagnostics().is_empty());
    }
  }

  #[test]
  fn preserves_static_record_order_and_keeps_projection_out_of_the_pass() {
    let modules = module_table(vec![
      normal_module(0, false, Vec::new()),
      normal_module(
        1,
        false,
        vec![
          (ImportKind::Import, Some(2), Span::new(10, 11)),
          (ImportKind::Require, Some(3), Span::new(20, 21)),
          (ImportKind::Import, Some(4), Span::new(30, 31)),
          (ImportKind::DynamicImport, Some(5), Span::new(40, 41)),
        ],
      ),
      normal_module(2, false, Vec::new()),
      normal_module(3, false, Vec::new()),
      external_module(4, "external"),
      normal_module(5, false, Vec::new()),
    ]);
    let entry_plan = entry_plan(&modules, &[1]);
    let mut pipeline = PassPipelineCtx::new();
    let (orders, sorted) = run_infallible_pass(
      ComputeModuleExecutionOrderPass,
      &mut pipeline,
      ComputeModuleExecutionOrderInput {
        module_table: &modules,
        entry_plan: &entry_plan,
        runtime: module_idx(0),
        code_splitting_disabled: false,
        check_circular_dependencies: false,
      },
      (),
    );

    assert_sealed(&orders);
    assert_eq!(
      orders.orders.iter_enumerated().map(|(idx, order)| (idx, *order)).collect::<Vec<_>>(),
      vec![
        (module_idx(0), 0),
        (module_idx(1), 4),
        (module_idx(2), 1),
        (module_idx(3), 2),
        (module_idx(4), 3),
        (module_idx(5), u32::MAX),
      ]
    );
    assert_eq!(
      orders.assigned().collect::<Vec<_>>(),
      vec![
        (module_idx(0), 0),
        (module_idx(1), 4),
        (module_idx(2), 1),
        (module_idx(3), 2),
        (module_idx(4), 3),
      ]
    );
    assert_eq!(
      sorted.into_inner(),
      vec![module_idx(0), module_idx(2), module_idx(3), module_idx(1)]
    );
    assert!(modules.modules.iter().all(|module| module.exec_order() == u32::MAX));
    assert!(pipeline.into_diagnostics().is_empty());
  }
}
