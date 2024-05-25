use oxc::{
  ast::ast::Program,
  semantic::{ScopeTree, Semantic, SemanticBuilder, SymbolTable},
  span::SourceType,
};

use crate::OxcAst;

impl OxcAst {
  pub fn is_body_empty(&self) -> bool {
    self.inner.with_dependent(|_, program| program.body.is_empty())
  }

  pub fn make_semantic<'ast>(
    source: &'ast str,
    program: &'_ Program<'ast>,
    ty: SourceType,
  ) -> Semantic<'ast> {
    let semantic = SemanticBuilder::new(source, ty).build(program).semantic;
    semantic
  }

  pub fn make_symbol_table_and_scope_tree(&self) -> (SymbolTable, ScopeTree) {
    self.inner.with_dependent(|dep, program| {
      let semantic = Self::make_semantic(&dep.source, program, self.source_type);
      semantic.into_symbol_table_and_scope_tree()
    })
  }
}
