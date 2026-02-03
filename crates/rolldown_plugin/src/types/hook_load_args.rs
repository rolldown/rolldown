use rolldown_common::ModuleIdx;

#[derive(Debug)]
pub struct HookLoadArgs<'a> {
  pub id: &'a str,
  pub module_idx: ModuleIdx,
}
