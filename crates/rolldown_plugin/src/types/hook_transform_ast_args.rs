use rolldown_common::ModuleType;
use rolldown_ecmascript::EcmaAst;

#[derive(Debug)]
pub struct HookTransformAstArgs<'a> {
  pub ast: EcmaAst,
  // The absolute path of the module, it is un-slashed.
  pub id: &'a str,
  // The relative path of the module, it is slashed.
  pub stable_id: &'a str,
  pub module_type: &'a ModuleType,
}
