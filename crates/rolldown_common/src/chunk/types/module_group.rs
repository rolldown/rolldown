use crate::ModuleIdx;

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
}
