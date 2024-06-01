use oxc::allocator::Allocator;
use rolldown_common::AstScopes;

mod impl_visit_mut;

pub struct IsolatingModuleFinalizer<'me, 'ast> {
  pub scope: &'me AstScopes,
  pub alloc: &'ast Allocator,
}
