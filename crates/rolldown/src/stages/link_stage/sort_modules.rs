use std::iter;

use rolldown_common::ModuleId;
use rustc_hash::{FxHashMap, FxHashSet};

use super::LinkStage;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Status {
  ToBeExecuted(ModuleId),
  WaitForExit(ModuleId),
}

impl<'a> LinkStage<'a> {
  pub fn sort_modules(&mut self) {
    // The runtime module should always be the first module to be executed
    let mut execution_stack = self
      .entries
      .iter()
      .rev()
      .map(|entry| Status::ToBeExecuted(entry.id.into()))
      .chain(iter::once(Status::ToBeExecuted(self.runtime.id().into())))
      .collect::<Vec<_>>();

    let mut stack_indexes_of_executing_id = FxHashMap::default();
    let mut executed_ids = FxHashSet::default();
    executed_ids
      .shrink_to(self.module_table.normal_modules.len() + self.module_table.external_modules.len());

    let mut sorted_modules = Vec::with_capacity(self.module_table.normal_modules.len());
    let mut next_exec_order = 0;
    let mut circular_dependencies = FxHashSet::default();
    while let Some(status) = execution_stack.pop() {
      match status {
        Status::ToBeExecuted(id) => {
          if executed_ids.contains(&id) {
            if let Some(index) = stack_indexes_of_executing_id.get(&id).copied() {
              // Executing
              let cycles = execution_stack[index..]
                .iter()
                .filter_map(|action| match action {
                  // Only modules with `Status::WaitForExit` are on the execution chain
                  Status::ToBeExecuted(_) => None,
                  Status::WaitForExit(id) => Some(*id),
                })
                .chain(iter::once(id))
                .collect::<Box<[_]>>();
              circular_dependencies.insert(cycles);
            } else {
              // It's already executed in other import chain, no need to execute again
            }
          } else {
            executed_ids.insert(id);
            execution_stack.push(Status::WaitForExit(id));
            debug_assert!(
              !stack_indexes_of_executing_id.contains_key(&id),
              "A module should not be executing the same module twice"
            );
            stack_indexes_of_executing_id.insert(id, execution_stack.len() - 1);

            if let ModuleId::Normal(module_id) = id {
              let module = &self.module_table.normal_modules[module_id];
              execution_stack.extend(
                module
                  .import_records
                  .iter()
                  .filter(|rec| rec.kind.is_static())
                  .map(|rec| rec.resolved_module)
                  .rev()
                  .map(Status::ToBeExecuted),
              );
            }
          }
        }
        Status::WaitForExit(id) => {
          executed_ids.insert(id);
          match id {
            ModuleId::Normal(id) => {
              let module = &mut self.module_table.normal_modules[id];
              debug_assert!(module.exec_order == u32::MAX);
              module.exec_order = next_exec_order;
              sorted_modules.push(id);
            }
            ModuleId::External(id) => {
              let module = &mut self.module_table.external_modules[id];
              debug_assert!(module.exec_order == u32::MAX);
              module.exec_order = next_exec_order;
            }
          }
          next_exec_order += 1;
          debug_assert!(stack_indexes_of_executing_id.contains_key(&id));
          stack_indexes_of_executing_id.remove(&id);
        }
      }
    }

    if !circular_dependencies.is_empty() {
      let mut cycles = circular_dependencies.into_iter().collect::<Vec<_>>();
      cycles.sort();
      // TODO: emit warning
    }

    self.sorted_modules = sorted_modules;
    debug_assert_eq!(
      self.sorted_modules.first().copied(),
      Some(self.runtime.id()),
      "runtime module should always be the first module in the sorted modules"
    );
  }
}
