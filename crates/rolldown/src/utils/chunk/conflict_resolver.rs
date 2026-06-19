use core::cmp::Reverse;

use oxc_str::CompactStr;
use rolldown_common::{ModuleTable, SymbolRef, SymbolRefDb};
use rolldown_utils::{concat_string, rustc_hash::FxHashMapExt};
use rustc_hash::FxHashMap;

/// Single source of the chunk `$N` conflict-suffix naming algorithm, shared by
/// chunk-internal deconfliction (`Renamer`) and cross-chunk export naming.
///
/// Owns the name -> conflict-index map. Callers store the returned name in their
/// own structure (`canonical_names`, `exports_to_other_chunks`, ...).
#[derive(Debug, Default)]
pub struct ConflictResolver {
  /// Key is a taken name; value is the conflict index used to generate the next
  /// unique `name$N`. See `renamer.rs` for the historical semantics this preserves.
  used: FxHashMap<CompactStr, u32>,
}

impl ConflictResolver {
  pub fn new(reserved: impl IntoIterator<Item = CompactStr>) -> Self {
    Self { used: reserved.into_iter().map(|name| (name, 0)).collect() }
  }

  /// Like `new` but pre-sizes the backing map and starts empty — for callers
  /// (e.g. the cross-chunk export pass) that know the name count up front and
  /// seed no reserved names.
  pub fn with_capacity(capacity: usize) -> Self {
    Self { used: FxHashMap::with_capacity(capacity) }
  }

  /// Reserve a name so it is never handed out (and seeds its conflict index at 0).
  /// Idempotent: re-reserving an existing name is a no-op that preserves its conflict index.
  pub fn reserve(&mut self, name: CompactStr) {
    self.used.entry(name).or_insert(0);
  }

  pub fn contains(&self, name: &str) -> bool {
    self.used.contains_key(name)
  }

  /// Pick a unique name for `base`, trying `base` then `base$1`, `base$2`, ...
  ///
  /// `accept(candidate, is_original)` vetoes a candidate that is otherwise free
  /// (e.g. would capture a nested binding). `is_original` is `true` only on the
  /// bare-`base` fast path. Returns the chosen name and records it as used.
  #[inline]
  pub fn resolve(&mut self, base: CompactStr, accept: impl Fn(&str, bool) -> bool) -> CompactStr {
    // Fast path: bare name is free and accepted. `base` is moved straight through
    // to the caller, so the name it already owns is never re-allocated.
    if !self.used.contains_key(&base) && accept(&base, true) {
      self.used.insert(base.clone(), 0);
      return base;
    }

    // Slow path: find `base$N`. `conflict_index` jumps past occupied candidates
    // by reading their stored counter (preserves renamer.rs:217-225).
    let mut conflict_index = self
      .used
      .get_mut(&base)
      .map(|idx| {
        *idx += 1;
        *idx
      })
      .unwrap_or(1);

    loop {
      let candidate: CompactStr =
        concat_string!(base, "$", itoa::Buffer::new().format(conflict_index)).into();

      if let Some(idx) = self.used.get_mut(&candidate) {
        *idx += 1;
        conflict_index = *idx;
        continue;
      }

      if !accept(&candidate, false) {
        conflict_index += 1;
        continue;
      }

      self.used.insert(candidate.clone(), 0);
      return candidate;
    }
  }
}

/// Canonical ordering for handing out deconflicted names: the entry module gets
/// naming priority. `exec_order` is assigned ascending (dependencies first,
/// entry last / highest), so `Reverse(exec_order)` sorts the entry first — the
/// same priority as the `.rev()` pass in `deconflict_chunk_symbols`. Ties are
/// broken by symbol name ascending. This is the single source of the ordering
/// invariant previously duplicated as the pinned-SHA comments in
/// `compute_cross_chunk_links.rs`.
pub fn deconflict_order_key<'a>(
  symbol_ref: SymbolRef,
  module_table: &ModuleTable,
  symbol_db: &'a SymbolRefDb,
) -> (Reverse<u32>, &'a str) {
  let exec_order = module_table[symbol_ref.owner].exec_order();
  (Reverse(exec_order), symbol_ref.name(symbol_db))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn cs(s: &str) -> CompactStr {
    CompactStr::new(s)
  }

  #[test]
  fn bare_name_when_free() {
    let mut r = ConflictResolver::new(std::iter::empty());
    assert_eq!(r.resolve(cs("foo"), |_, _| true).as_str(), "foo");
  }

  #[test]
  fn suffix_on_collision() {
    let mut r = ConflictResolver::new(std::iter::empty());
    assert_eq!(r.resolve(cs("foo"), |_, _| true).as_str(), "foo");
    assert_eq!(r.resolve(cs("foo"), |_, _| true).as_str(), "foo$1");
    assert_eq!(r.resolve(cs("foo"), |_, _| true).as_str(), "foo$2");
  }

  #[test]
  fn reserved_names_are_avoided() {
    let mut r = ConflictResolver::new([cs("foo")]);
    assert_eq!(r.resolve(cs("foo"), |_, _| true).as_str(), "foo$1");
  }

  #[test]
  fn accept_veto_skips_candidate() {
    // Reject any name equal to "foo$1" (e.g. simulating a nested-capture veto),
    // forcing the loop to advance to "foo$2".
    let mut r = ConflictResolver::new(std::iter::empty());
    assert_eq!(r.resolve(cs("foo"), |_, _| true).as_str(), "foo"); // occupies "foo"
    assert_eq!(r.resolve(cs("foo"), |cand, _is_original| cand != "foo$1").as_str(), "foo$2");
  }

  #[test]
  fn jump_uses_stored_counter_of_occupied_candidate() {
    // Reproduces renamer.rs:221-225: when a candidate is already used, its stored
    // counter is read and the index jumps past it.
    let mut r = ConflictResolver::new(std::iter::empty());
    r.reserve(cs("foo"));
    r.reserve(cs("foo$1"));
    // base "foo" occupied -> try "foo$1" (occupied, counter 0 -> becomes 1, jump to "foo$2")
    assert_eq!(r.resolve(cs("foo"), |_, _| true).as_str(), "foo$2");
  }

  #[test]
  fn jump_walks_multi_hop_chain() {
    // Exercises a three-hop jump: foo -> foo$1 -> foo$2 -> foo$3.
    // All three candidates are pre-reserved; the index must walk the full chain.
    let mut r = ConflictResolver::new(std::iter::empty());
    r.reserve(cs("foo"));
    r.reserve(cs("foo$1"));
    r.reserve(cs("foo$2"));
    assert_eq!(r.resolve(cs("foo"), |_, _| true).as_str(), "foo$3");
  }

  #[test]
  fn contains_reports_reserved() {
    let mut r = ConflictResolver::new([cs("a")]);
    assert!(r.contains("a"));
    assert!(!r.contains("b"));
    r.reserve(cs("b"));
    assert!(r.contains("b"));
  }

  #[test]
  fn order_key_is_reverse_exec_order_then_name() {
    // The key type: the entry module has the highest exec_order (dependencies
    // execute first with lower orders, entry last). Reverse sorts the highest
    // exec_order first (entry-first), and ties break by name ascending. We
    // assert the tuple shape and comparison semantics with synthetic values.
    use core::cmp::Reverse;
    let a = (Reverse(5u32), CompactStr::new("b"));
    let b = (Reverse(5u32), CompactStr::new("a"));
    let c = (Reverse(6u32), CompactStr::new("z"));
    let mut v = vec![a.clone(), b.clone(), c.clone()];
    v.sort();
    // c has higher exec_order -> Reverse puts it first; within exec_order 5, "a" < "b".
    assert_eq!(v, vec![c, b, a]);
  }
}
