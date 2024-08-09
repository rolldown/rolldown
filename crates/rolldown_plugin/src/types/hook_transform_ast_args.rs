use std::path::PathBuf;

use rolldown_ecmascript::EcmaAst;

#[derive(Debug)]
pub struct HookTransformAstArgs<'a> {
  // TODO: id?
  pub cwd: &'a PathBuf,
  pub ast: EcmaAst,
}
