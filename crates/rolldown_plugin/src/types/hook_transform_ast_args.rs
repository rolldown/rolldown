use std::path::PathBuf;

use rolldown_ecmascript::EcmaAst;

#[derive(Debug)]
pub struct HookTransformAstArgs<'a> {
  pub cwd: &'a PathBuf,
  pub ast: EcmaAst,
  pub id: &'a str,
  pub is_user_defined_entry: bool,
}
