use super::TakeIn;

use oxc::allocator::{Allocator, Box, Vec};

impl<'ast, T: TakeIn<'ast>> TakeIn<'ast> for Box<'ast, T> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Box::new_in(TakeIn::dummy(alloc), alloc)
  }
}

impl<'ast, T> TakeIn<'ast> for Vec<'ast, T> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Vec::new_in(alloc)
  }
}
