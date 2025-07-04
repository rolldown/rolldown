use oxc::{ast::ast::Expression, minifier::PropertyReadSideEffects, semantic::Scoping};
use oxc_ecmascript::{
  constant_evaluation::{ConstantEvaluation, ConstantEvaluationCtx},
  is_global_reference::IsGlobalReference,
  side_effects::MayHaveSideEffectsContext,
};
use rolldown_common::ConstantValue;

pub struct ConstEvalCtx<'me, 'ast: 'me> {
  pub ast: oxc::ast::AstBuilder<'ast>,
  pub scope: &'me Scoping,
}

impl<'ast> ConstantEvaluationCtx<'ast> for ConstEvalCtx<'_, 'ast> {
  fn ast(&self) -> oxc::ast::AstBuilder<'ast> {
    self.ast
  }
}

impl<'ast> IsGlobalReference<'ast> for ConstEvalCtx<'_, 'ast> {
  fn is_global_reference(
    &self,
    reference: &oxc::ast::ast::IdentifierReference<'ast>,
  ) -> Option<bool> {
    let item = self.scope.get_reference(reference.reference_id());
    Some(item.symbol_id().is_none())
  }
}

impl<'ast> MayHaveSideEffectsContext<'ast> for ConstEvalCtx<'_, 'ast> {
  fn annotations(&self) -> bool {
    false
  }

  fn manual_pure_functions(&self, _callee: &Expression) -> bool {
    true
  }

  fn property_read_side_effects(&self) -> oxc::minifier::PropertyReadSideEffects {
    PropertyReadSideEffects::All
  }

  fn unknown_global_side_effects(&self) -> bool {
    true
  }
}

pub fn try_extract_const_literal<'me, 'ast: 'me>(
  ctx: &ConstEvalCtx<'me, 'ast>,
  expr: &Expression<'ast>,
) -> Option<ConstantValue> {
  expr.evaluate_value(ctx).map(ConstantValue::from)
}
