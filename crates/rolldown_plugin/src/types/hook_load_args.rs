use rolldown_common::{ModuleIdx, ModuleType};

#[derive(Debug)]
pub struct HookLoadArgs<'a> {
  pub id: &'a str,
  pub module_idx: ModuleIdx,
  pub asserted_module_type: Option<&'a ModuleType>,
}
