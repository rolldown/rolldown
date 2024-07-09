use oxc::allocator::Allocator;
use rolldown_common::{EcmaModule, IndexEcmaModules};
use rolldown_ecmascript::AstSnippet;

mod impl_visit_mut;

pub struct IsolatingModuleFinalizerContext<'me> {
  pub module: &'me EcmaModule,
  pub modules: &'me IndexEcmaModules,
}

pub struct IsolatingModuleFinalizer<'me, 'ast> {
  pub ctx: &'me IsolatingModuleFinalizerContext<'me>,
  // pub scope: &'me AstScopes,
  pub alloc: &'ast Allocator,
  pub snippet: AstSnippet<'ast>,
}
