use arcstr::ArcStr;
use rolldown_common::{ImportAttributes, ModuleType};

#[derive(Debug)]
pub struct HookTransformArgs<'a> {
  pub id: &'a str,
  pub raw_id: &'a str,
  pub attributes: &'a ImportAttributes,
  pub code: &'a ArcStr,
  pub module_type: &'a ModuleType,
}
