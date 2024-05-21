use oxc::{
  ast::ast::Program,
  semantic::{ScopeTree, Semantic, SemanticBuilder, SymbolTable},
  span::SourceType,
};

use crate::{allocator_helpers::take_in::TakeIn, OxcAst, StatementExt};

use super::WithFieldsMut;

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
      let semantic = Self::make_semantic(&dep.0, program, self.source_type);
      semantic.into_symbol_table_and_scope_tree()
    })
  }

  // TODO: should move this to `rolldown` crate
  pub fn hoist_import_export_from_stmts(&mut self) {
    self.with_mut(|WithFieldsMut { program, allocator, .. }| {
      let (hoisted, non_hoisted): (Vec<_>, Vec<_>) = program
        .body
        .take_in(allocator)
        .into_iter()
        .partition(StatementExt::is_module_declaration_with_source);

      program.body.reserve_exact(hoisted.len() + non_hoisted.len());
      program.body.extend(hoisted);
      program.body.extend(non_hoisted);
    });
  }
}
