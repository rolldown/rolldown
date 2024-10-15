use oxc::{
  ast::ast::Program,
  semantic::{ScopeTree, Semantic, SemanticBuilder, SymbolTable},
};

use crate::EcmaAst;

impl EcmaAst {
  pub fn is_body_empty(&self) -> bool {
    self.program().is_empty()
  }

  pub fn make_semantic<'ast>(program: &'_ Program<'ast>) -> Semantic<'ast> {
    let semantic = SemanticBuilder::new().with_scope_tree_child_ids(true).build(program).semantic;
    semantic
  }

  pub fn make_symbol_table_and_scope_tree(&self) -> (SymbolTable, ScopeTree) {
    self.program.with_dependent(|_owner, dep| {
      let semantic = Self::make_semantic(&dep.program);
      semantic.into_symbol_table_and_scope_tree()
    })
  }
}
