use rolldown_common::ModuleId;
use rustc_hash::FxHashSet;

use super::LinkStage;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Action {
  Enter(ModuleId),
  Exit(ModuleId),
}

impl<'a> LinkStage<'a> {
  pub fn sort_modules(&mut self) {
    tracing::trace!("Start sort modules");
    let mut stack = self
      .entries
      .iter()
      .map(|entry_point| Action::Enter(entry_point.id.into()))
      .rev()
      .collect::<Vec<_>>();
    // The runtime module should always be the first module to be executed
    stack.push(Action::Enter(self.runtime.id().into()));
    let mut entered_ids = FxHashSet::default();
    entered_ids
      .shrink_to(self.module_table.normal_modules.len() + self.module_table.external_modules.len());
    let mut sorted_modules = Vec::with_capacity(self.module_table.normal_modules.len());
    let mut next_exec_order = 0;
    while let Some(action) = stack.pop() {
      match action {
        Action::Enter(id) => {
          if !entered_ids.contains(&id) {
            entered_ids.insert(id);
            stack.push(Action::Exit(id));
            if let ModuleId::Normal(module_id) = id {
              let module = &self.module_table.normal_modules[module_id];
              stack.extend(
                module
                  .import_records
                  .iter()
                  .filter(|rec| rec.kind.is_static())
                  .map(|rec| rec.resolved_module)
                  .rev()
                  .map(Action::Enter),
              );
            }
          }
        }
        Action::Exit(id) => {
          match id {
            ModuleId::Normal(id) => {
              let module = &mut self.module_table.normal_modules[id];
              module.exec_order = next_exec_order;
              sorted_modules.push(id);
            }
            ModuleId::External(id) => {
              let module = &mut self.module_table.external_modules[id];
              module.exec_order = next_exec_order;
            }
          }
          next_exec_order += 1;
        }
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
