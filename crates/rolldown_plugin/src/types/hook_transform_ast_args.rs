use std::path::PathBuf;

use oxc::semantic::{ScopeTree, SymbolTable};
use rolldown_ecmascript::EcmaAst;

#[derive(Debug)]
pub struct HookTransformAstArgs<'a> {
  pub cwd: &'a PathBuf,
  pub ast: EcmaAst,
  pub id: &'a str,
  pub is_user_defined_entry: bool,
  pub symbols: &'a mut SymbolTable,
  pub scopes: &'a mut ScopeTree,
  pub ast_changed: &'a mut bool,
}
