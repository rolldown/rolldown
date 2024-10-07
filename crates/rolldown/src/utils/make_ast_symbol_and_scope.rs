use oxc::semantic::{ScopeTree, SymbolTable};
use rolldown_common::AstScopes;

pub fn make_ast_scopes_and_symbols(
  symbols: SymbolTable,
  scopes: ScopeTree,
) -> (SymbolTable, AstScopes) {
  let mut symbols = symbols;
  let ast_scope = AstScopes::new(
    scopes,
    std::mem::take(&mut symbols.references),
    std::mem::take(&mut symbols.resolved_references),
  );
  (symbols, ast_scope)
}
