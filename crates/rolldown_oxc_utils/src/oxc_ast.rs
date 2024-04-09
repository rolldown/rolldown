use std::{fmt::Debug, sync::Arc};

use crate::{OxcCompiler, StatementExt, TakeIn};
use oxc::{
  allocator::Allocator,
  ast::ast::Program,
  semantic::{ScopeTree, Semantic, SemanticBuilder, SymbolTable},
  span::SourceType,
};

use self_cell::self_cell;

self_cell!(
  pub(crate) struct Inner {
    owner: (Arc<str>, Allocator),

    #[not_covariant]
    dependent: Program,
  }
);

pub struct OxcAst {
  pub(crate) inner: Inner,
}
impl Debug for OxcAst {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Ast").field("source", &self.inner.borrow_owner().0).finish_non_exhaustive()
  }
}

impl Default for OxcAst {
  fn default() -> Self {
    OxcCompiler::parse("", SourceType::default())
  }
}

unsafe impl Send for OxcAst {}
unsafe impl Sync for OxcAst {}

pub struct WithFields<'outer, 'inner> {
  pub source: &'inner Arc<str>,
  pub allocator: &'inner Allocator,
  pub program: &'outer Program<'inner>,
}

pub struct WithFieldsMut<'outer, 'inner> {
  pub source: &'inner Arc<str>,
  pub allocator: &'inner Allocator,
  pub program: &'outer mut Program<'inner>,
}

impl OxcAst {
  pub fn source(&self) -> &Arc<str> {
    &self.inner.borrow_owner().0
  }

  pub fn is_body_empty(&self) -> bool {
    self.inner.with_dependent(|_, program| program.body.is_empty())
  }

  pub fn with<'outer, Ret>(
    &'outer self,
    func: impl for<'inner> ::core::ops::FnOnce(WithFields<'outer, 'inner>) -> Ret,
  ) -> Ret {
    self.inner.with_dependent::<'outer, Ret>(|owner, program| {
      func(WithFields { source: &owner.0, allocator: &owner.1, program })
    })
  }

  pub fn with_mut<'outer, Ret>(
    &'outer mut self,
    func: impl for<'inner> ::core::ops::FnOnce(WithFieldsMut<'outer, 'inner>) -> Ret,
  ) -> Ret {
    self.inner.with_dependent_mut::<'outer, Ret>(|owner, program| {
      func(WithFieldsMut { source: &owner.0, allocator: &owner.1, program })
    })
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
      // FIXME: Should not use default source type
      let semantic = Self::make_semantic(&dep.0, program, SourceType::default());
      semantic.into_symbol_table_and_scope_tree()
    })
  }

  // TODO: should move this to `rolldown` crate
  pub fn hoist_import_export_from_stmts(&mut self) {
    self.inner.with_dependent_mut(|dep, program| {
      let (program, allocator) = (program, &dep.1);
      let old_body = program.body.take_in(allocator);
      program.body.reserve_exact(old_body.len());
      let mut non_hoisted = oxc::allocator::Vec::new_in(allocator);

      old_body.into_iter().for_each(|top_stmt| {
        if top_stmt.is_module_declaration_with_source() {
          program.body.push(top_stmt);
        } else {
          non_hoisted.push(top_stmt);
        }
      });
      program.body.extend(non_hoisted);
    });
  }
}
