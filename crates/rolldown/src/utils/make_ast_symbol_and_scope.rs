use oxc::semantic::{ScopeTree, SymbolTable};
use rolldown_common::AstScopes;

pub fn make_ast_scopes_and_symbols(
  symbols: SymbolTable,
  scopes: ScopeTree,
) -> (SymbolTable, AstScopes) {
  let symbols = symbols;
  let ast_scope = AstScopes::new(scopes);
  (symbols, ast_scope)
}
