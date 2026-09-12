use oxc::{
  allocator::{Allocator, GetAllocator},
  ast::{
    ast::{Expression, TemplateLiteral},
    builder::GetAstBuilder,
  },
  minifier::PropertyReadSideEffects,
  semantic::{IsGlobalReference, ReferenceId, Scoping, SymbolId},
};
use oxc_ecmascript::{
  GlobalContext, ToJsString, ValueType,
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
    Expression::TemplateLiteral(template) => {
      template_text(ctx, template).map(ConstantValue::String)
    }
    _ => expr.evaluate_value(ctx).map(ConstantValue::from),
  }
}

/// The text of an untagged template whose substitutions all evaluate, or `None`.
///
/// oxc's `evaluate_value` has no arm for template literals, so `` const a = `` `` was never a
/// constant and `` `${a}` `` stayed a possibly-throwing coercion (#10817). Each substitution is
/// evaluated with the constant map, so `` `${NAME}!` `` folds when `NAME` is a known constant;
/// oxc's own `ToJsString` would only fold the globals `undefined`, `NaN` and `Infinity` there.
/// Like oxc, this bails on lone surrogates: a new string literal built from the text would print
/// their `\u{FFFD}` escape encoding as literal text.
fn template_text<'ast>(
  ctx: &ConstEvalCtx<'_, 'ast>,
  template: &TemplateLiteral<'ast>,
) -> Option<String> {
  let mut text = String::new();
  for (i, quasi) in template.quasis.iter().enumerate() {
    if quasi.lone_surrogates {
      return None;
    }
    text.push_str(quasi.value.cooked.as_ref()?);
    match template.expressions.get(i) {
      None => {}
      Some(Expression::TemplateLiteral(inner)) => text.push_str(&template_text(ctx, inner)?),
      Some(expr) => {
        let value = expr.evaluate_value_to(ctx, Some(ValueType::String))?;
        text.push_str(&value.to_js_string(ctx)?);
      }
    }
  }
  Some(text)
}

#[cfg(test)]
mod tests {
  use super::*;
  use oxc::{
    ast::ast::{BindingPattern, Statement},
    parser::Parser,
    semantic::SemanticBuilder,
    span::SourceType,
  };
  use rolldown_common::AstScopes;

  /// Evaluate the initializer of the root binding `target` in `code`. Every root binding declared
  /// before it whose initializer evaluates is already in the constant map, as in the scanner.
  fn extract(code: &str, target: &str) -> Option<ConstantValue> {
    let allocator = Allocator::default();
    let program = Parser::new(&allocator, code, SourceType::default()).parse().program;
    let ast_scopes = AstScopes::new(SemanticBuilder::new().build(&program).semantic.into_scoping());
    let mut constant_map = FxHashMap::default();
    for stmt in &program.body {
      let Statement::VariableDeclaration(decl) = stmt else { continue };
      for declarator in &decl.declarations {
        let BindingPattern::BindingIdentifier(id) = &declarator.id else { continue };
        let value = {
          let ctx = ConstEvalCtx {
            ast: oxc::ast::builder::AstBuilder::new(&allocator),
            scope: ast_scopes.scoping(),
            constant_map: &constant_map,
            overrode_get_constant_value_from_reference_id: None,
          };
          declarator.init.as_ref().and_then(|init| try_extract_const_literal(&ctx, init))
        };
        if id.name == target {
          return value;
        }
        if let Some(value) = value {
          constant_map.insert(id.symbol_id(), ConstExportMeta::new(value, false));
        }
      }
    }
    panic!("`{target}` is not a root binding of {code:?}")
  }

  fn string(text: &str) -> ConstantValue {
    ConstantValue::String(text.to_string())
  }

  #[test]
  fn test_template_literal_folds_to_a_string_constant() {
    assert_eq!(extract("const a = ``;", "a"), Some(string("")));
    assert_eq!(extract("const a = `plain`;", "a"), Some(string("plain")));
    assert_eq!(extract("const a = `x${1}y${true}${null}`;", "a"), Some(string("x1ytruenull")));
    // A substitution that is a known constant folds through the constant map. oxc's own
    // `ToJsString` stops at the identifier.
    assert_eq!(
      extract("const NAME = 'rolldown'; const a = `${NAME}!`;", "a"),
      Some(string("rolldown!"))
    );
    assert_eq!(
      extract("const SECOND = 1000; const a = `${SECOND}ms`;", "a"),
      Some(string("1000ms"))
    );
    assert_eq!(extract("const BIG = 1e21; const a = `${BIG}`;", "a"), Some(string("1e+21")));
    assert_eq!(
      extract("const NAME = 'rolldown'; const a = `${NAME + '!'}`;", "a"),
      Some(string("rolldown!"))
    );
    assert_eq!(
      extract("const NAME = 'rolldown'; const a = `${`${NAME}`}!`;", "a"),
      Some(string("rolldown!"))
    );
  }

  #[test]
  fn test_template_literal_with_an_unknown_substitution_is_not_a_constant() {
    assert_eq!(extract("const a = `${x}`;", "a"), None);
    assert_eq!(extract("let x; const a = `${x}`;", "a"), None);
    assert_eq!(extract("const a = `${foo()}`;", "a"), None);
    assert_eq!(extract("const a = `${a}`;", "a"), None);
    assert_eq!(extract("const NAME = 'rolldown'; const a = `${NAME}${x}`;", "a"), None);
    // A lone surrogate cannot be carried into a new string literal.
    assert_eq!(extract("const a = `\\uD800`;", "a"), None);
  }
}
