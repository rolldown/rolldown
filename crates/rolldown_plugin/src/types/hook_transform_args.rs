use rolldown_common::ModuleType;

#[derive(Debug)]
pub struct HookTransformArgs<'a> {
  pub id: &'a str,
  pub code: &'a String,
  pub module_type: &'a ModuleType,
}
