use oxc::{
  allocator::{Allocator, Box},
  ast::ast,
};

use crate::{from_in::FromIn, Dummy};

pub trait IntoIn<'ast, T>: Sized {
  fn into_in(self, alloc: &'ast Allocator) -> T;
}

impl<'ast, T> IntoIn<'ast, Box<'ast, T>> for T {
  fn into_in(self, alloc: &'ast Allocator) -> Box<'ast, T> {
    Box(alloc.alloc(self))
  }
}

impl<'ast, T: FromIn<'ast, T>> IntoIn<'ast, T> for T {
  fn into_in(self, alloc: &'ast Allocator) -> T {
    T::from_in(self, alloc)
  }
}

impl<'ast> IntoIn<'ast, ast::Statement<'ast>> for ast::Expression<'ast> {
  fn into_in(self, alloc: &'ast Allocator) -> ast::Statement<'ast> {
    ast::Statement::ExpressionStatement(
      ast::ExpressionStatement { expression: self, ..Dummy::dummy(alloc) }.into_in(alloc),
    )
  }
}
