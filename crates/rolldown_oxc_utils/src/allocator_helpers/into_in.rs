use oxc::{
  allocator::{Allocator, Box},
  ast::ast,
};

use super::{from_in::FromIn, take_in::TakeIn};

pub trait IntoIn<'ast, T>: Sized {
  fn into_in(self, alloc: &'ast Allocator) -> T;
}

impl<'ast, T> IntoIn<'ast, Box<'ast, T>> for T {
  fn into_in(self, alloc: &'ast Allocator) -> Box<'ast, T> {
    Box::new_in(self, alloc)
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
      ast::ExpressionStatement { expression: self, ..TakeIn::dummy(alloc) }.into_in(alloc),
    )
  }
}
