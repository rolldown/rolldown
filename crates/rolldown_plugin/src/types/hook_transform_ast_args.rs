use std::path::PathBuf;

use rolldown_oxc_utils::OxcAst;

#[derive(Debug)]
pub struct HookTransformAstArgs<'a> {
  pub cwd: &'a PathBuf,
  pub ast: OxcAst,
}
