use oxc::semantic::{ScopeTree, SymbolTable};
use rolldown_common::AstScopes;

use crate::types::ast_symbols::AstSymbols;

pub fn make_ast_scopes_and_symbols(
  symbols: SymbolTable,
  scopes: ScopeTree,
) -> (AstSymbols, AstScopes) {
  let mut symbols = symbols;
  let ast_scope = AstScopes::new(
    scopes,
    std::mem::take(&mut symbols.references),
    std::mem::take(&mut symbols.resolved_references),
  );
  let ast_symbols = AstSymbols::from_symbol_table(symbols);
  (ast_symbols, ast_scope)
}
