use oxc::{
  ast::ast::Program,
  semantic::{ScopeTree, Semantic, SemanticBuilder, SymbolTable},
  span::SourceType,
};

use crate::EcmaAst;

impl EcmaAst {
  pub fn is_body_empty(&self) -> bool {
    self.program().is_empty()
  }

  pub fn make_semantic<'ast>(
    source: &'ast str,
    program: &'_ Program<'ast>,
    ty: SourceType,
  ) -> Semantic<'ast> {
    let build_result =
      SemanticBuilder::new(source, ty).with_check_syntax_error(true).build(program);
    // TODO: log errors and warnings.
    println!("BUILD RESULT - {:?}", build_result.errors);
    build_result.semantic
  }

  pub fn make_symbol_table_and_scope_tree(&self) -> (SymbolTable, ScopeTree) {
    self.program.with_dependent(|owner, dep| {
      let semantic = Self::make_semantic(&owner.source, &dep.program, self.source_type);
      semantic.into_symbol_table_and_scope_tree()
    })
  }
}
