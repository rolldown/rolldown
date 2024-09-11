use oxc::{allocator::Allocator, ast::ast::ObjectPropertyKind, span::CompactStr};
use rolldown_common::{AstScopes, EcmaModule, IndexModules};
use rolldown_ecmascript::AstSnippet;
use rustc_hash::FxHashSet;

use crate::types::symbols::Symbols;

mod impl_visit_mut;

pub struct IsolatingModuleFinalizerContext<'me> {
  pub module: &'me EcmaModule,
  pub modules: &'me IndexModules,
  pub symbols: &'me Symbols,
}

pub struct IsolatingModuleFinalizer<'me, 'ast> {
  pub ctx: &'me IsolatingModuleFinalizerContext<'me>,
  pub scope: &'me AstScopes,
  pub alloc: &'ast Allocator,
  pub snippet: AstSnippet<'ast>,
  pub generated_imports: FxHashSet<CompactStr>,
  pub generated_exports: oxc::allocator::Vec<'ast, ObjectPropertyKind<'ast>>,
}
