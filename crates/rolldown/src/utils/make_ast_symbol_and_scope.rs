use rolldown_common::AstScopes;
use rolldown_oxc_utils::OxcAst;

use crate::types::ast_symbols::AstSymbols;

pub fn make_ast_scopes_and_symbols(ast: &OxcAst) -> (AstScopes, AstSymbols) {
  let (mut symbol_table, scope) = ast.make_symbol_table_and_scope_tree();
  let ast_scope = AstScopes::new(
    scope,
    std::mem::take(&mut symbol_table.references),
    std::mem::take(&mut symbol_table.resolved_references),
  );
  let ast_symbols = AstSymbols::from_symbol_table(symbol_table);
  (ast_scope, ast_symbols)
}
