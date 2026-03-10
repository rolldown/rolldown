use oxc::ast::ast::{CallExpression, Expression, IdentifierReference};
use oxc_allocator::{Address, UnstableAddress};
use oxc_ecmascript::side_effects::PropertyReadSideEffects;
use oxc_ecmascript::{GlobalContext, side_effects::MayHaveSideEffectsContext};
use rolldown_common::{AstScopes, FlatOptions, SharedNormalizedBundlerOptions};
use rustc_hash::FxHashSet;

use super::extract_first_part_of_member_expr_like;

/// Bridge type implementing Oxc's `MayHaveSideEffectsContext` trait using
/// real bundler options. This enables gradual migration from Rolldown's
/// `SideEffectDetector` to Oxc's `MayHaveSideEffects` trait.
pub struct BundlerSideEffectCtx<'a> {
  pub scope: &'a AstScopes,
  pub options: &'a SharedNormalizedBundlerOptions,
  pub flat_options: FlatOptions,
  /// Cross-module optimization: addresses of call expressions to known-pure functions.
  side_effect_free_call_expr_addr: Option<&'a FxHashSet<Address>>,
}

impl<'a> BundlerSideEffectCtx<'a> {
  pub fn new(
    scope: &'a AstScopes,
    options: &'a SharedNormalizedBundlerOptions,
    flat_options: FlatOptions,
    side_effect_free_call_expr_addr: Option<&'a FxHashSet<Address>>,
  ) -> Self {
    Self { scope, options, flat_options, side_effect_free_call_expr_addr }
  }

  /// Check if a call expression has been marked pure by cross-module optimization.
  pub fn is_call_expr_marked_pure(&self, expr: &CallExpression) -> bool {
    self.side_effect_free_call_expr_addr
      .is_some_and(|set| set.contains(&expr.unstable_address()))
  }
}

impl GlobalContext<'_> for BundlerSideEffectCtx<'_> {
  fn is_global_reference(&self, reference: &IdentifierReference<'_>) -> bool {
    self.scope.is_unresolved(reference.reference_id.get().unwrap())
  }
}

impl MayHaveSideEffectsContext<'_> for BundlerSideEffectCtx<'_> {
  fn annotations(&self) -> bool {
    !self.flat_options.ignore_annotations()
  }

  fn manual_pure_functions(&self, callee: &Expression) -> bool {
    if self.flat_options.is_manual_pure_functions_empty() {
      return false;
    }
    let manual_pure_functions = self.options.treeshake.manual_pure_functions().unwrap();
    let Some(first_part) = extract_first_part_of_member_expr_like(callee) else {
      return false;
    };
    manual_pure_functions.contains(first_part)
  }

  fn property_read_side_effects(&self) -> PropertyReadSideEffects {
    if self.flat_options.property_read_side_effects() {
      PropertyReadSideEffects::All
    } else {
      PropertyReadSideEffects::None
    }
  }

  fn unknown_global_side_effects(&self) -> bool {
    self.options.treeshake.unknown_global_side_effects()
  }

  fn property_write_side_effects(&self) -> bool {
    self.flat_options.property_write_side_effects()
  }
}
