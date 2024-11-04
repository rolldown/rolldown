use oxc::{allocator::Allocator, syntax};

use super::TakeIn;

impl<'ast> TakeIn<'ast> for syntax::operator::UnaryOperator {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Self::Void
  }
}

impl<'ast> TakeIn<'ast> for syntax::operator::AssignmentOperator {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Self::Assign
  }
}
