use oxc::semantic::SymbolId;

use crate::{ModuleIdx, SymbolRef};

use super::member_expr_ref::MemberExprRef;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymbolOrMemberExprRef {
  Symbol(SymbolRef),
  MemberExpr(MemberExprRef),
}

impl SymbolOrMemberExprRef {
  /// get the first part of the expression,
  /// e.g. `test.a.b` will return `test`
  /// for identifier, it will return itself
  pub fn symbol_ref(&self) -> &SymbolRef {
    match self {
      SymbolOrMemberExprRef::Symbol(s) => s,
      SymbolOrMemberExprRef::MemberExpr(expr) => &expr.object_ref,
    }
  }
}

impl From<MemberExprRef> for SymbolOrMemberExprRef {
  fn from(value: MemberExprRef) -> Self {
    Self::MemberExpr(value)
  }
}

impl From<SymbolRef> for SymbolOrMemberExprRef {
  fn from(value: SymbolRef) -> Self {
    Self::Symbol(value)
  }
}

/// Bit of [`TaggedSymbolRef::symbol_with_flag`] that marks a link-only symbol.
const LINK_ONLY_FLAG: u32 = 1 << 31;

/// A [`SymbolRef`] plus a one-bit "link-only" tag, packed into the same 8 bytes as a bare
/// `SymbolRef` (instead of the 12 bytes a `enum { LinkOnly(SymbolRef), Normal(SymbolRef) }`
/// would cost, since two payload-carrying variants force a separate, 4-byte-aligned discriminant).
///
/// A *link-only* symbol is only used during linking and never generates an actual binding in the
/// output, e.g. the facade symbols created for CJS named exports so that
/// ```js
/// exports['foo'] = 1;
/// ```
/// can participate in tree-shaking even though `foo` has no real declaration in the original code.
///
/// The tag lives in the high bit of the `SymbolId`. This is sound because a single module can never
/// reach `2^31` symbols: oxc caps a source file at <4GB (`u32` spans) and the densest binding that
/// still introduces distinct symbols (`a=>a=>…`, 3 bytes each) tops out around `1.4e9`, well under
/// `2^31`; the handful of bundler-created facade symbols don't change that. [`Self::pack`] asserts
/// the invariant so a pathological input panics instead of silently corrupting a symbol id.
#[derive(Clone, Copy)]
pub struct TaggedSymbolRef {
  owner: ModuleIdx,
  /// Low 31 bits: the `SymbolId`. High bit (`LINK_ONLY_FLAG`): set iff this is a link-only symbol.
  symbol_with_flag: u32,
}

const _: () = assert!(size_of::<TaggedSymbolRef>() == size_of::<SymbolRef>());

impl TaggedSymbolRef {
  fn pack(symbol_ref: SymbolRef, link_only: bool) -> Self {
    let raw = symbol_ref.symbol.raw().get();
    assert!(
      (raw & LINK_ONLY_FLAG) == 0,
      "SymbolId {raw} reaches 2^31; the high bit is needed to tag link-only symbols"
    );
    Self {
      owner: symbol_ref.owner,
      symbol_with_flag: if link_only { raw | LINK_ONLY_FLAG } else { raw },
    }
  }

  pub fn normal(symbol_ref: SymbolRef) -> Self {
    Self::pack(symbol_ref, false)
  }

  pub fn link_only(symbol_ref: SymbolRef) -> Self {
    Self::pack(symbol_ref, true)
  }

  pub fn is_link_only(&self) -> bool {
    (self.symbol_with_flag & LINK_ONLY_FLAG) != 0
  }

  pub fn is_normal(&self) -> bool {
    !self.is_link_only()
  }

  pub fn inner(&self) -> SymbolRef {
    SymbolRef {
      owner: self.owner,
      symbol: SymbolId::from_usize((self.symbol_with_flag & !LINK_ONLY_FLAG) as usize),
    }
  }
}

impl std::fmt::Debug for TaggedSymbolRef {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TaggedSymbolRef")
      .field("inner", &self.inner())
      .field("link_only", &self.is_link_only())
      .finish()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn symbol_ref(owner: u32, symbol: u32) -> SymbolRef {
    SymbolRef::from((ModuleIdx::from_raw(owner), SymbolId::from_usize(symbol as usize)))
  }

  #[test]
  fn normal_roundtrips() {
    let s = symbol_ref(7, 42);
    let tagged = TaggedSymbolRef::normal(s);
    assert!(tagged.is_normal());
    assert!(!tagged.is_link_only());
    assert_eq!(tagged.inner(), s);
  }

  #[test]
  fn link_only_roundtrips() {
    let s = symbol_ref(7, 42);
    let tagged = TaggedSymbolRef::link_only(s);
    assert!(tagged.is_link_only());
    assert!(!tagged.is_normal());
    assert_eq!(tagged.inner(), s);
  }

  #[test]
  fn largest_taggable_symbol_id_roundtrips() {
    // The biggest symbol id that still leaves the tag bit (`1 << 31`) free.
    let s = symbol_ref(12345, (1u32 << 31) - 1);
    assert_eq!(TaggedSymbolRef::normal(s).inner(), s);
    let link = TaggedSymbolRef::link_only(s);
    assert!(link.is_link_only());
    assert_eq!(link.inner(), s);
  }

  #[test]
  #[should_panic(expected = "the high bit is needed")]
  fn panics_when_symbol_id_uses_tag_bit() {
    // A symbol id with bit 31 set would collide with the link-only tag.
    let _ = TaggedSymbolRef::normal(symbol_ref(0, 1u32 << 31));
  }

  #[test]
  fn same_size_as_symbol_ref() {
    assert_eq!(size_of::<TaggedSymbolRef>(), size_of::<SymbolRef>());
    assert_eq!(size_of::<TaggedSymbolRef>(), 8);
  }
}
