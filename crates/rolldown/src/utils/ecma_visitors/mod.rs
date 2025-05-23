use oxc::{
  ast_visit::{VisitMut, walk_mut},
  span::{GetSpanMut, SPAN, Span},
};
use rustc_hash::FxHashSet;

/// Make sure there aren't any duplicate spans in the AST.
pub struct EnsureSpanUniqueness {
  // visited_spans: FxHashMap</* start */ u32, /* ends */ FxHashSet<u32>>,
  visited_spans: FxHashSet<Span>,
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

  fn visit_this_expression(&mut self, it: &mut oxc::ast::ast::ThisExpression) {
    self.ensure_uniqueness(it.span_mut());
    walk_mut::walk_this_expression(self, it);
  }

  fn visit_call_expression(&mut self, it: &mut oxc::ast::ast::CallExpression<'a>) {
    if it.callee.is_specific_id("require") && it.arguments.len() == 1 {
      self.ensure_uniqueness(it.span_mut());
    }
    walk_mut::walk_call_expression(self, it);
  }

  fn visit_new_expression(&mut self, it: &mut oxc::ast::ast::NewExpression<'a>) {
    self.ensure_uniqueness(it.span_mut());
    walk_mut::walk_new_expression(self, it);
  }
  fn visit_identifier_reference(&mut self, it: &mut oxc::ast::ast::IdentifierReference<'a>) {
    if it.name == "require" {
      self.ensure_uniqueness(it.span_mut());
    }
  }
}

impl EnsureSpanUniqueness {
  pub fn new() -> Self {
    Self { visited_spans: FxHashSet::from_iter([SPAN]), next_unique_span_start: 1 }
  }

  fn ensure_uniqueness(&mut self, span: &mut Span) {
    if self.visited_spans.contains(span) {
      *span = self.generate_unique_span();
    }
    self.visited_spans.insert(*span);
  }

  fn generate_unique_span(&mut self) -> Span {
    let mut span_candidate = Span::new(self.next_unique_span_start, self.next_unique_span_start);
    while self.visited_spans.contains(&span_candidate) {
      self.next_unique_span_start += 1;
      span_candidate = Span::new(self.next_unique_span_start, self.next_unique_span_start);
    }
    debug_assert!(span_candidate.is_empty());
    span_candidate
  }
}
