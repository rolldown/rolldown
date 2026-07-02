//! Tree-shaking inclusion policy.
//!
//! The generic reachability walk in [`super::include_statements`]
//! (`include_module` / `include_statement` / `include_symbol`) asks the questions
//! here instead of encoding domain rules inline. Each function is a pure read over
//! [`IncludeContext`]; keeping them together lets the walk read as a plain graph
//! traversal and lets a rule change (or a new rule) live in one place.

use rolldown_common::{MemberExprRef, Module, SymbolRef};

use super::include_statements::{IncludeContext, SymbolIncludeReason};

/// Whether `canonical_ref` is an inlinable constant that is substituted at every use
/// site, so its declaration need not be included.
///
/// In smart mode we only skip when `safe_to_inline` (it will be inlined regardless of
/// context). CommonJS module exports are never skipped. When `inlineConst` is disabled
/// the map is empty, so this is always `false`.
pub fn symbol_is_inlined_const(
  ctx: &IncludeContext<'_>,
  canonical_ref: SymbolRef,
  include_reason: SymbolIncludeReason,
) -> bool {
  ctx.constant_symbol_map.get(&canonical_ref).is_some_and(|v| {
    !include_reason.contains(SymbolIncludeReason::EntryExport)
      && (!ctx.inline_const_smart || v.safe_to_inline)
      && !v.commonjs_export
  })
}

/// Whether `member_expr_ref` is an enum member access (`E.member`) that the finalizer
/// will inline to a literal, so the enum declaration need not be included for this
/// reference.
///
/// Only simple accesses qualify — not deep chains like `E.member.something`, which are
/// not inlined. Enum inlining is unconditional (not gated by `inlineConst`) because it
/// implements TypeScript's const-enum semantics, which mandate replacement. A bare
/// reference to the enum elsewhere (`typeof E`, `console.log(E)`) still includes the
/// declaration via its own `include_symbol`.
pub fn member_is_inlined_enum(ctx: &IncludeContext<'_>, member_expr_ref: &MemberExprRef) -> bool {
  let canonical_ref = ctx.symbols.canonical_ref_for(member_expr_ref.object_ref);
  let Some(Module::Normal(owner_module)) = ctx.modules.get(canonical_ref.owner) else {
    return false;
  };
  let symbol_name = canonical_ref.name(ctx.symbols);
  let Some(members) = owner_module.ecma_view.enum_member_value_map.get(symbol_name) else {
    return false;
  };
  !member_expr_ref.is_write
    && matches!(
      member_expr_ref.prop_and_span_list.as_slice(),
      [prop] if members.contains_key(prop.name.as_str())
    )
}
