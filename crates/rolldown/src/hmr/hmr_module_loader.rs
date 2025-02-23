use std::sync::Arc;

use arcstr::ArcStr;
use rolldown_common::{Module, ModuleIdx, ModuleLoaderMsg, ModuleTable};
use rolldown_error::BuildResult;
use rustc_hash::FxHashMap;

use crate::{SharedOptions, module_loader::task_context::TaskContext};

#[expect(unused)]
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
  #[expect(unused, clippy::unnecessary_wraps)]
  pub fn run(&mut self, changed_module_idx: Vec<ModuleIdx>) -> BuildResult<()> {
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

    // TODO: Use changed_module_ids as starting point to find new modules

    Ok(())
  }
}
