use oxc::{
  ast::ast::Expression,
  minifier::PropertyReadSideEffects,
  semantic::{IsGlobalReference, Scoping, SymbolId},
};
use oxc_ecmascript::{
  GlobalContext,
  constant_evaluation::{ConstantEvaluation, ConstantEvaluationCtx},
  side_effects::MayHaveSideEffectsContext,
};
use rolldown_common::{ConstExportMeta, ConstantValue};
use rustc_hash::FxHashMap;

pub struct ConstEvalCtx<'me, 'ast: 'me> {
  pub ast: oxc::ast::AstBuilder<'ast>,
  pub scope: &'me Scoping,
  pub constant_map: &'me FxHashMap<SymbolId, ConstExportMeta>,
}

impl<'ast> ConstantEvaluationCtx<'ast> for ConstEvalCtx<'_, 'ast> {
  fn ast(&self) -> oxc::ast::AstBuilder<'ast> {
    self.ast
  }
}

impl<'ast> GlobalContext<'ast> for ConstEvalCtx<'_, 'ast> {
  fn is_global_reference(&self, reference: &oxc::ast::ast::IdentifierReference<'ast>) -> bool {
    reference.is_global_reference(self.scope)
  }

  fn get_constant_value_for_reference_id(
    &self,
    reference_id: oxc::semantic::ReferenceId,
  ) -> Option<oxc_ecmascript::constant_evaluation::ConstantValue<'ast>> {
    let reference = self.scope.get_reference(reference_id);
    let symbol_id = reference.symbol_id()?;
    let v = self.constant_map.get(&symbol_id)?;
    Some(oxc_ecmascript::constant_evaluation::ConstantValue::from(&v.value))
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
