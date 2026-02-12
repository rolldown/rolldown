use oxc::ast::ast::{CallExpression, ChainElement, Expression, IdentifierReference, Statement};
use oxc_allocator::{Address, UnstableAddress};
use oxc_ecmascript::{
  GlobalContext,
  side_effects::{
    MayHaveSideEffects, MayHaveSideEffectsContext, PropertyReadSideEffects,
    PropertyWriteSideEffects,
  },
};
use rolldown_common::{AstScopes, FlatOptions, SharedNormalizedBundlerOptions, SideEffectDetail};
use rustc_hash::FxHashSet;

pub struct SideEffectContext<'a> {
  scope: &'a AstScopes,
  flat_options: FlatOptions,
  options: &'a SharedNormalizedBundlerOptions,
  side_effect_free_function_symbol_ref: Option<&'a FxHashSet<Address>>,
}

impl<'a> SideEffectContext<'a> {
  pub fn new(
    scope: &'a AstScopes,
    flat_options: FlatOptions,
    options: &'a SharedNormalizedBundlerOptions,
    side_effect_free_function_symbol_ref: Option<&'a FxHashSet<Address>>,
  ) -> Self {
    Self { scope, flat_options, options, side_effect_free_function_symbol_ref }
  }

  pub fn detect_stmt<'ast>(&self, stmt: &Statement<'ast>) -> SideEffectDetail
  where
    Self: MayHaveSideEffectsContext<'ast>,
  {
    stmt.side_effect_detail(self)
  }

  fn extract_first_part_of_member_expr_like<'ast>(expr: &Expression<'ast>) -> Option<&'ast str> {
    let mut cur = expr;
    loop {
      match cur {
        Expression::Identifier(ident) => break Some(ident.name.as_str()),
        Expression::ComputedMemberExpression(expr) => {
          cur = &expr.object;
        }
        Expression::StaticMemberExpression(expr) => {
          cur = &expr.object;
        }
        Expression::CallExpression(expr) => {
          cur = &expr.callee;
        }
        Expression::ChainExpression(expr) => match expr.expression {
          ChainElement::CallExpression(ref call_expression) => {
            cur = &call_expression.callee;
          }
          ChainElement::ComputedMemberExpression(ref computed_member_expression) => {
            cur = &computed_member_expression.object;
          }
          ChainElement::StaticMemberExpression(ref static_member_expression) => {
            cur = &static_member_expression.object;
          }
          ChainElement::TSNonNullExpression(_) | ChainElement::PrivateFieldExpression(_) => {
            break None;
          }
        },
        _ => break None,
      }
    }
  }
}

impl<'ast> GlobalContext<'ast> for SideEffectContext<'_> {
  fn is_global_reference(&self, reference: &IdentifierReference<'ast>) -> bool {
    let Some(reference_id) = reference.reference_id.get() else {
      return false;
    };
    self.scope.is_unresolved(reference_id)
  }
}

impl<'ast> MayHaveSideEffectsContext<'ast> for SideEffectContext<'_> {
  fn annotations(&self) -> bool {
    !self.flat_options.ignore_annotations()
  }

  fn manual_pure_functions(&self, callee: &Expression) -> bool {
    if self.flat_options.is_manual_pure_functions_empty() {
      return false;
    }

    let Some(first_part) = Self::extract_first_part_of_member_expr_like(callee) else {
      return false;
    };
    self
      .options
      .treeshake
      .manual_pure_functions()
      .is_some_and(|manual_pure_functions| manual_pure_functions.contains(first_part))
  }

  fn property_read_side_effects(&self) -> PropertyReadSideEffects {
    if self.flat_options.property_read_side_effects() {
      PropertyReadSideEffects::All
    } else {
      PropertyReadSideEffects::None
    }
  }

  fn property_write_side_effects(&self) -> PropertyWriteSideEffects {
    if self.flat_options.property_write_side_effects() {
      PropertyWriteSideEffects::All
    } else {
      PropertyWriteSideEffects::None
    }
  }

  fn unknown_global_side_effects(&self) -> bool {
    self.options.treeshake.unknown_global_side_effects()
  }

  fn is_pure_call_expression(&self, expr: &CallExpression<'ast>) -> bool {
    self
      .side_effect_free_function_symbol_ref
      .is_some_and(|map| map.contains(&expr.unstable_address()))
  }
}
