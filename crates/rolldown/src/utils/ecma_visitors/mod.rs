use oxc::{
  ast::{visit::walk_mut, VisitMut},
  span::{GetSpanMut, Span},
};

/// Make sure there aren't any duplicate spans in the AST.
pub struct EnsureSpanUniqueness {
  next_unique_span_start: u32,
}

impl<'a> VisitMut<'a> for EnsureSpanUniqueness {
  fn visit_program(&mut self, it: &mut oxc::ast::ast::Program<'a>) {
    self.next_unique_span_start = it.span.end + 1;
    walk_mut::walk_program(self, it);
  }

  // TODO: it's better use `visit_span`, but it's not implemented yet by oxc. https://github.com/oxc-project/oxc/issues/4799
  fn visit_module_declaration(&mut self, it: &mut oxc::ast::ast::ModuleDeclaration<'a>) {
    self.ensure_uniqueness(it.span_mut());
    walk_mut::walk_module_declaration(self, it);
  }

  fn visit_import_expression(&mut self, it: &mut oxc::ast::ast::ImportExpression<'a>) {
    self.ensure_uniqueness(it.span_mut());
    walk_mut::walk_import_expression(self, it);
  }
}

impl EnsureSpanUniqueness {
  pub fn new() -> Self {
    Self { next_unique_span_start: u32::MAX }
  }

  fn ensure_uniqueness(&mut self, span: &mut Span) {
    if span.is_empty() {
      *span = self.generate_unique_span();
    }
  }

  fn generate_unique_span(&mut self) -> Span {
    self.next_unique_span_start -= 1;
    let span_candidate = self.next_unique_span_start;
    debug_assert_ne!(span_candidate, 0);
    Span::new(span_candidate, span_candidate)
  }
}
