use std::sync::Arc;

use arcstr::ArcStr;
use rolldown_common::{Module, ModuleId, ModuleIdx, ModuleLoaderMsg, ModuleTable, ResolvedId};
use rustc_hash::FxHashMap;

use crate::{
  module_loader::{
    module_task::{ModuleTask, ModuleTaskOwner},
    task_context::TaskContext,
  },
  SharedOptions,
};

pub struct HmrModuleLoader<'me> {
  options: SharedOptions,
  shared_context: Arc<TaskContext>,
  rx: tokio::sync::mpsc::Receiver<ModuleLoaderMsg>,
  remaining: u32,
  pub module_db: &'me ModuleTable,
  pub fetched_modules: FxHashMap<ArcStr, ModuleIdx>,
  pub remaining_tasks: u32,
}

impl HmrModuleLoader<'_> {
  pub fn run(&mut self, changed_module_idx: Vec<ModuleIdx>) {
    let mut changed_module_ids = vec![];
    for changed_module_idx in changed_module_idx {
      let Module::Normal(module) = &self.module_db.modules[changed_module_idx] else {
        continue;
      };
      changed_module_ids.push(module.id.clone());
    }

    changed_module_ids.iter().for_each(|id| {
      self.fetched_modules.remove(id.resource_id());
    });

    for changed_module_id in changed_module_ids {}
  }

  fn try_spawn_new_task(
    &mut self,
    resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
  ) -> ModuleIdx {
    match self.fetched_modules.entry(resolved_id.id.clone()) {
      std::collections::hash_map::Entry::l(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        if resolved_id.is_external {
          unreachable!("External modules should not be fetched in HMR");
        } else {
          self.remaining_tasks += 1;
          let idx = self.intermediate_normal_modules.alloc_ecma_module_idx();
          not_visited.insert(idx);
          self.remaining += 1;

          let task = ModuleTask::new(Arc::clone(&self.shared_context), idx, resolved_id, owner);
          #[cfg(target_family = "wasm")]
          {
            let handle = tokio::runtime::Handle::current();
            // could not block_on/spawn the main thread in WASI
            std::thread::spawn(move || {
              handle.spawn(task.run());
            });
          }
          #[cfg(not(target_family = "wasm"))]
          tokio::spawn(task.run());
          idx
        }
      }
    }
  }
}
