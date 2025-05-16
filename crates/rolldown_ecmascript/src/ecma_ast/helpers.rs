use oxc::{
  ast::ast::Program,
  semantic::{Scoping, Semantic, SemanticBuilder},
};

use crate::EcmaAst;

impl EcmaAst {
  pub fn is_body_empty(&self) -> bool {
    self.program().is_empty()
  }

  pub fn make_semantic<'ast>(program: &'ast Program<'ast>, with_cfg: bool) -> Semantic<'ast> {
    SemanticBuilder::new()
      .with_scope_tree_child_ids(true)
      .with_cfg(with_cfg)
      .build(program)
      .semantic
  }

  pub fn make_scoping(&self) -> Scoping {
    self.program.with_dependent(|_owner, dep| {
      Self::make_semantic(&dep.program, /*with_cfg*/ false).into_scoping()
    })
  }

  pub fn make_symbol_table_and_scope_tree_with_semantic_builder<'a>(
    &'a self,
    semantic_builder: SemanticBuilder<'a>,
  ) -> Scoping {
    self.program.with_dependent::<'a, Scoping>(|_owner, dep| {
      semantic_builder.build(&dep.program).semantic.into_scoping()
    })
  }
}
