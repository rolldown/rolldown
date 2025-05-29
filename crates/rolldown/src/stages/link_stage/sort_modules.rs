use std::iter;

use rolldown_common::{ImportRecordMeta, Module, ModuleIdx};
use rolldown_error::{BuildDiagnostic, EventKindSwitcher};
use rolldown_utils::rustc_hash::FxHashSetExt;
use rustc_hash::{FxHashMap, FxHashSet};

use super::LinkStage;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Status {
  ToBeExecuted(ModuleIdx),
  WaitForExit(ModuleIdx),
}

impl LinkStage<'_> {
  /// Some notes about the module execution order:
  /// - We assume user-defined entries are always executed orderly.
  /// - Async entries is sorted by `Module#debug_id` of entry module to ensure deterministic output.
  /// - `require(...)` is treated as implicit static `import`, which required modules are executed before the module that requires them.
  /// - Since import statements are hoisted, `require(...)` is always placed after static `import` statements.
  /// - Order of `require(...)` is determined by who shows up first while scanning ast. For such code
  ///
  /// ```js
  /// () => require('b')
  /// require('c')
  /// import 'a';
  /// ```
  ///
  /// The execution order is `a -> b -> c`.
  /// - We only ensure execution order is relative correct, which means imported/required modules are executed before the module that imports/require them.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn sort_modules(&mut self) {
    // The runtime module should always be the first module to be executed
    let mut execution_stack = self
      .entries
      .iter()
      .rev()
      .map(|entry| Status::ToBeExecuted(entry.id))
      .chain(iter::once(Status::ToBeExecuted(self.runtime.id())))
      .collect::<Vec<_>>();

    let mut next_exec_order = 0;

    let mut executed_ids = FxHashSet::with_capacity(self.module_table.modules.len());
    let mut stack_indexes_of_executing_id = FxHashMap::default();

    let mut sorted_modules = Vec::with_capacity(self.module_table.modules.len());
    let mut circular_dependencies = FxHashSet::default();

    while let Some(status) = execution_stack.pop() {
      match status {
        Status::ToBeExecuted(id) => {
          if executed_ids.contains(&id) {
            if self.options.checks.contains(EventKindSwitcher::CircularDependency) {
              // Try to check if there is a circular dependency
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
              }
            }
            // It's already executed in other import chain, no need to execute again
          } else {
            executed_ids.insert(id);
            execution_stack.push(Status::WaitForExit(id));
            debug_assert!(
              !stack_indexes_of_executing_id.contains_key(&id),
              "A module should not be executing the same module twice"
            );
            stack_indexes_of_executing_id.insert(id, execution_stack.len() - 1);

            // top level await module should be sorted.
            execution_stack.extend(
              self.module_table[id]
                .import_records()
                .iter()
                .filter(|rec| {
                  rec.kind.is_static()
                    || rec.meta.contains(ImportRecordMeta::IS_TOP_LEVEL_AWAIT_DYNAMIC_IMPORT)
                })
                .map(|rec| rec.resolved_module)
                .rev()
                .map(Status::ToBeExecuted),
            );
          }
        }
        Status::WaitForExit(id) => {
          match &mut self.module_table[id] {
            Module::Normal(module) => {
              debug_assert!(module.exec_order == u32::MAX);
              module.exec_order = next_exec_order;
              sorted_modules.push(id);
            }
            Module::External(module) => {
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
      for cycle in circular_dependencies {
        let paths = cycle
          .iter()
          .filter_map(|id| {
            self.module_table[*id].as_normal().map(|module| module.id.to_string())
          })
          .collect::<Vec<_>>();
        self.warnings.push(BuildDiagnostic::circular_dependency(paths).with_severity_warning());
      }
    }

    self.sorted_modules = sorted_modules;
    debug_assert_eq!(
      self.sorted_modules.first().copied(),
      Some(self.runtime.id()),
      "runtime module should always be the first module in the sorted modules"
    );
  }
}
