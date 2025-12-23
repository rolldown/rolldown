use oxc::{ast::ast::Expression, semantic::SymbolFlags, span::Span};
use rolldown_common::{Specifier, SymbolRef};
use rolldown_ecmascript_utils::ExpressionExt;
use rolldown_error::BuildDiagnostic;

use super::AstScanner;

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  /// Check if a namespace import is being called as a function.
  /// If so, emit a warning diagnostic.
  pub fn check_namespace_call(&mut self, callee: &Expression<'ast>, callee_span: Span) {
    let Some(symbol_id) =
      callee.as_identifier().and_then(|ident| self.resolve_symbol_from_reference(ident))
    else {
      return;
    };
    if !self.is_root_symbol(symbol_id) {
      return;
    }

    let symbol_flag = self.result.symbol_ref_db.scoping().symbol_flags(symbol_id);
    if symbol_flag.contains(SymbolFlags::Import) {
      let symbol_ref: SymbolRef = (self.immutable_ctx.idx, symbol_id).into();
      let is_namespace = self
        .result
        .named_imports
        .get(&symbol_ref)
        .is_some_and(|import| matches!(import.imported, Specifier::Star));
      if is_namespace {
        let name = self.result.symbol_ref_db.symbol_name(symbol_id);
        self.result.warnings.push(
          BuildDiagnostic::cannot_call_namespace(
            self.immutable_ctx.id.resource_id().clone(),
            self.immutable_ctx.source.clone(),
            callee_span,
            name.into(),
          )
          .with_severity_warning(),
        );
      }
    }
  }
}
