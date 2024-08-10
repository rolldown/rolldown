use std::path::{Path, PathBuf};

use rolldown_ecmascript::EcmaAst;

#[derive(Debug)]
pub struct HookTransformAstArgs<'a> {
  pub path: &'a Path,
  pub cwd: &'a PathBuf,
  pub ast: EcmaAst,
}
