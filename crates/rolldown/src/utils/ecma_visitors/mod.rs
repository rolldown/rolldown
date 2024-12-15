use oxc::{
  ast::{visit::walk_mut, VisitMut},
  span::{Span, SPAN},
};
use rustc_hash::FxHashSet;

/// Make sure there aren't any duplicate spans in the AST.
pub struct EnsureSpanUniqueness {
  visited_spans: FxHashSet<Span>,
  next_unique_span_start: u32,
}

impl EnsureSpanUniqueness {
  pub fn new() -> Self {
    Self { visited_spans: FxHashSet::from_iter(vec![SPAN]), next_unique_span_start: 1 }
  }
}

impl<'a> VisitMut<'a> for EnsureSpanUniqueness {
  fn visit_program(&mut self, it: &mut oxc::ast::ast::Program<'a>) {
    self.next_unique_span_start = it.span.end + 1;
    walk_mut::walk_program(self, it);
  }

  #[inline]
  fn visit_span(&mut self, it: &mut Span) {
    if it.start == it.end {
      if self.visited_spans.contains(it) {
        let mut span_candidate =
          Span::new(self.next_unique_span_start, self.next_unique_span_start);
        while self.visited_spans.contains(&span_candidate) {
          self.next_unique_span_start += 1;
          span_candidate = Span::new(self.next_unique_span_start, self.next_unique_span_start);
        }

        *it = span_candidate;
        self.next_unique_span_start += 1;
      }
    }
    self.visited_spans.insert(*it);
    // We don’t need to walk it since it’s logically empty,
    // but to prevent future changes in oxc, we follow the semantics.
    walk_mut::walk_span(self, it);
  }
}
