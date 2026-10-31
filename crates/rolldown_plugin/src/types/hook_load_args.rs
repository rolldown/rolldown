use rolldown_common::{ImportAttributes, ModuleIdx, ModuleType};

#[derive(Debug)]
pub struct HookLoadArgs<'a> {
  pub id: &'a str,
  pub raw_id: &'a str,
  pub attributes: &'a ImportAttributes,
  pub module_idx: ModuleIdx,
  pub asserted_module_type: Option<&'a ModuleType>,
}
