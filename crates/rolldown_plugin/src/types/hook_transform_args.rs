use arcstr::ArcStr;
use rolldown_common::ModuleType;

#[derive(Debug)]
pub struct HookTransformArgs<'a> {
  pub id: &'a str,
  pub code: &'a ArcStr,
  pub module_type: &'a ModuleType,
}
