use oxc::allocator::Allocator;
use rolldown_common::AstScope;

mod impl_visit_mut;

pub struct IsolatingModuleFinalizer<'me, 'ast> {
  pub scope: &'me AstScope,
  pub alloc: &'ast Allocator,
}
