use oxc::{
  ast::{visit::walk_mut, VisitMut},
  span::{Span, SPAN},
};
use rustc_hash::FxHashSet;

/// Make sure there aren't any duplicate spans in the AST.
pub struct EnsureSpanUniqueness {
  visited_ids: FxHashSet<u32>,
  next_unique_id: u32,
}

impl EnsureSpanUniqueness {
  pub fn new() -> Self {
    Self { visited_ids: FxHashSet::from_iter(vec![SPAN.start]), next_unique_id: 1 }
  }
}

impl<'a> VisitMut<'a> for EnsureSpanUniqueness {
  #[inline]
  fn visit_span(&mut self, it: &mut Span) {
    if it.start == it.end {
      if self.visited_ids.contains(&it.start) {
        while self.visited_ids.contains(&self.next_unique_id) {
          self.next_unique_id += 1;
        }

        *it = Span::new(self.next_unique_id, self.next_unique_id);
        self.next_unique_id += 1;
      }
      self.visited_ids.insert(it.start);
    }
    // We don’t need to walk it since it’s logically empty,
    // but to prevent future changes in oxc, we follow the semantics.
    walk_mut::walk_span(self, it);
  }
}
