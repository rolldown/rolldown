use crate::ModuleIdx;
#[cfg(debug_assertions)]
use crate::ModuleTable;

#[derive(Debug, Clone)]
pub struct ModuleGroup {
  pub modules: Vec<ModuleIdx>,
  // TODO: maybe we could remove this since the entry module of a module group
  // should always be the last module after the module group sorted by execution order.
  pub entry: ModuleIdx,
}

impl ModuleGroup {
  pub fn new(modules: Vec<ModuleIdx>, entry: ModuleIdx) -> Self {
    Self { modules, entry }
  }

  #[cfg(debug_assertions)]
  #[track_caller]
  #[allow(clippy::print_stdout)]
  pub fn debug_module_group(&self, module_table: &ModuleTable) {
    let caller = std::panic::Location::caller();
    println!("[{}:{}] Debugging module group:", caller.file(), caller.line());
    for module_idx in &self.modules {
      let module = &module_table[*module_idx];
      println!("{}", &module.stable_id());
    }
  }
}
