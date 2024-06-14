use oxc::allocator::Allocator;
use rolldown_common::{NormalModule, NormalModuleVec};
use rolldown_oxc_utils::AstSnippet;

mod impl_visit_mut;

pub struct IsolatingModuleFinalizerContext<'me> {
  pub module: &'me NormalModule,
  pub modules: &'me NormalModuleVec,
}

pub struct IsolatingModuleFinalizer<'me, 'ast> {
  pub ctx: &'me IsolatingModuleFinalizerContext<'me>,
  // pub scope: &'me AstScopes,
  pub alloc: &'ast Allocator,
  pub snippet: AstSnippet<'ast>,
}
