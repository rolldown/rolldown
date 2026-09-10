use oxc::{
  allocator::{Allocator, GetAllocator},
  ast::{ast::Expression, builder::GetAstBuilder},
  minifier::PropertyReadSideEffects,
  semantic::{IsGlobalReference, ReferenceId, Scoping, SymbolId},
};
use oxc_ecmascript::{
  GlobalContext, ToJsString,
  constant_evaluation::{ConstantEvaluation, ConstantEvaluationCtx},
  side_effects::MayHaveSideEffectsContext,
};
use rolldown_common::{ConstExportMeta, ConstantValue};
use rustc_hash::FxHashMap;

pub struct ConstEvalCtx<'me, 'ast: 'me> {
  pub ast: oxc::ast::builder::AstBuilder<'ast>,
  pub scope: &'me Scoping,
  pub constant_map: &'me FxHashMap<SymbolId, ConstExportMeta>,
  pub overrode_get_constant_value_from_reference_id: Option<
    &'me dyn Fn(ReferenceId) -> Option<oxc_ecmascript::constant_evaluation::ConstantValue<'ast>>,
  >,
}

impl<'ast> ConstantEvaluationCtx<'ast> for ConstEvalCtx<'_, 'ast> {}

impl<'ast> GetAstBuilder<'ast> for ConstEvalCtx<'_, 'ast> {
  type Builder = oxc::ast::builder::AstBuilder<'ast>;

  #[inline]
  fn builder(&self) -> &oxc::ast::builder::AstBuilder<'ast> {
    &self.ast
  }
}

impl<'ast> GetAllocator<'ast> for ConstEvalCtx<'_, 'ast> {
  #[inline]
  fn allocator(&self) -> &'ast Allocator {
    self.ast.allocator()
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
    // If there is an override function, return the result produced by the overrode function
    // whatever.
    if let Some(f) = self.overrode_get_constant_value_from_reference_id {
      return f(reference_id);
    }
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
  match expr {
    // oxc's `evaluate_value` has no arm for template literals, so `` const a = `` `` was never
    // recorded as a constant and `` `${a}` `` stayed a possibly-throwing coercion (#10817). An
    // untagged template whose text folds is the string literal of that text; `to_js_string` is
    // what oxc folds `` `${1}` `` with and bails on lone surrogates like `evaluate_value` does.
    Expression::TemplateLiteral(template) => {
      template.to_js_string(ctx).map(|text| ConstantValue::String(text.into_owned()))
    }
    _ => expr.evaluate_value(ctx).map(ConstantValue::from),
  }
}
