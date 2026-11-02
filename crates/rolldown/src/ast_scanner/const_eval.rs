use oxc::{
  allocator::{Allocator, GetAllocator},
  ast::{
    ast::{Expression, TemplateLiteral},
    builder::GetAstBuilder,
  },
  ast_visit::{VisitJs, walk_js},
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
  pub overrode_get_constant_value_from_reference_id:
    Option<&'me dyn Fn(ReferenceId) -> Option<&'me ConstantValue>>,
}

impl<'me> ConstEvalCtx<'me, '_> {
  fn constant_value(&self, reference_id: ReferenceId) -> Option<&'me ConstantValue> {
    // If there is an override function, return the result produced by the overrode function
    // whatever.
    if let Some(f) = self.overrode_get_constant_value_from_reference_id {
      return f(reference_id);
    }
    let symbol_id = self.scope.get_reference(reference_id).symbol_id()?;
    self.constant_map.get(&symbol_id).map(|meta| &meta.value)
  }
}

impl<'me> ConstantEvaluationCtx<'me> for ConstEvalCtx<'me, '_> {}

impl<'me> GetAstBuilder<'me> for ConstEvalCtx<'me, '_> {
  type Builder = oxc::ast::builder::AstBuilder<'me>;

  #[inline]
  fn builder(&self) -> &oxc::ast::builder::AstBuilder<'me> {
    &self.ast
  }
}

impl<'me> GetAllocator<'me> for ConstEvalCtx<'me, '_> {
  #[inline]
  fn allocator(&self) -> &'me Allocator {
    self.ast.allocator()
  }
}

impl<'me> GlobalContext<'me> for ConstEvalCtx<'me, '_> {
  fn is_global_reference(&self, reference: &oxc::ast::ast::IdentifierReference<'me>) -> bool {
    reference.is_global_reference(self.scope)
  }

  fn get_constant_value_for_reference_id(
    &self,
    reference_id: oxc::semantic::ReferenceId,
  ) -> Option<oxc_ecmascript::constant_evaluation::ConstantValue<'me>> {
    self.constant_value(reference_id).map(oxc_ecmascript::constant_evaluation::ConstantValue::from)
  }

  // Without the type, oxc's `+` reads a string constant as a number: `'3' + 1` gives `4`.
  fn value_type_for_reference_id(&self, reference_id: ReferenceId) -> Option<ValueType> {
    self.constant_value(reference_id).map(ConstantValue::value_type)
  }
}

impl<'me> MayHaveSideEffectsContext<'me> for ConstEvalCtx<'me, '_> {
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
  let value = match expr {
    Expression::TemplateLiteral(template) => {
      template_text(ctx, template).map(ConstantValue::String)
    }
    _ => expr.evaluate_value(ctx).and_then(ConstantValue::from_oxc),
  }?;
  (!contains_object_literal(expr)).then_some(value)
}

/// Returns whether `expr` contains an array, object, or regexp literal.
///
/// oxc converts such a literal with the default `toString`, and keeps the source order of regexp
/// flags. A program can patch a prototype, and the runtime prints regexp flags in a fixed order.
fn contains_object_literal(expr: &Expression) -> bool {
  struct Finder(bool);
  impl<'a> VisitJs<'a> for Finder {
    fn visit_expression(&mut self, it: &Expression<'a>) {
      if self.0 {
        return;
      }
      self.0 = matches!(
        it,
        Expression::ArrayExpression(_)
          | Expression::ObjectExpression(_)
          | Expression::RegExpLiteral(_)
      );
      if !self.0 {
        walk_js::walk_expression(self, it);
      }
    }
  }
  let mut finder = Finder(false);
  finder.visit_expression(expr);
  finder.0
}

/// Returns whether direct `eval` in the module can assign `symbol_id`.
///
/// Direct `eval` can assign a `let`, a `var`, or an `exports` property without a write reference,
/// so only a `const` is stable in such a module.
pub fn eval_can_assign(scoping: &Scoping, symbol_id: SymbolId) -> bool {
  scoping.root_scope_flags().contains_direct_eval()
    && !scoping.symbol_flags(symbol_id).is_const_variable()
}

/// Returns the text of an untagged template. Returns `None` when a substitution does not evaluate
/// to a primitive constant or a text piece has a lone surrogate.
///
/// oxc's `evaluate_value` has no arm for template literals (#10817), and its `ToJsString` folds
/// only global identifiers. So this evaluates each substitution with the constant map.
fn template_text<'ast>(
  ctx: &ConstEvalCtx<'_, 'ast>,
  template: &TemplateLiteral<'ast>,
) -> Option<String> {
  let mut text = String::new();
  for (i, quasi) in template.quasis.iter().enumerate() {
    text.push_str(quasi.value.cooked?.as_str()?);
    match template.expressions.get(i) {
      None => {}
      Some(Expression::TemplateLiteral(inner)) => text.push_str(&template_text(ctx, inner)?),
      Some(expr) => {
        let value = expr.evaluate_value(ctx)?;
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

  /// Evaluate the initializer of the root binding `target` in `code`. Earlier root bindings that
  /// evaluate are in the constant map, as in the scanner.
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
    // `+` concatenates when one operand is a string constant.
    assert_eq!(extract("const NAME = '3'; const a = `${NAME + 1}`;", "a"), Some(string("31")));
    assert_eq!(extract("const NAME = '3'; const a = NAME + 1;", "a"), Some(string("31")));
  }

  #[test]
  fn test_string_constant_lookups_do_not_allocate_in_the_ast_arena() {
    let allocator = Allocator::default();
    let program = Parser::new(
      &allocator,
      "const VALUE = ''; const target = VALUE.length;",
      SourceType::default(),
    )
    .parse()
    .program;
    let ast_scopes = AstScopes::new(SemanticBuilder::new().build(&program).semantic.into_scoping());
    let Statement::VariableDeclaration(decl) = &program.body[0] else { unreachable!() };
    let BindingPattern::BindingIdentifier(binding) = &decl.declarations[0].id else {
      unreachable!()
    };
    let mut constant_map = FxHashMap::default();
    constant_map.insert(
      binding.symbol_id(),
      ConstExportMeta::new(ConstantValue::String("x".repeat(4096)), false),
    );
    let ctx = ConstEvalCtx {
      ast: oxc::ast::builder::AstBuilder::new(&allocator),
      scope: ast_scopes.scoping(),
      constant_map: &constant_map,
      overrode_get_constant_value_from_reference_id: None,
    };
    let Statement::VariableDeclaration(decl) = &program.body[1] else { unreachable!() };
    let expr = decl.declarations[0].init.as_ref().unwrap();
    let used_bytes = allocator.used_bytes();
    for _ in 0..128 {
      assert_eq!(try_extract_const_literal(&ctx, expr), Some(ConstantValue::Number(4096.0)));
    }
    assert_eq!(allocator.used_bytes(), used_bytes);
  }

  #[test]
  fn test_template_literal_with_an_unknown_substitution_is_not_a_constant() {
    assert_eq!(extract("const a = `${x}`;", "a"), None);
    assert_eq!(extract("let x; const a = `${x}`;", "a"), None);
    assert_eq!(extract("const a = `${foo()}`;", "a"), None);
    assert_eq!(extract("const a = `${a}`;", "a"), None);
    assert_eq!(extract("const NAME = 'rolldown'; const a = `${NAME}${x}`;", "a"), None);
    // A new string literal cannot carry a lone surrogate.
    assert_eq!(extract("const a = `\\uD800`;", "a"), None);
  }

  #[test]
  fn test_string_with_a_lone_surrogate_is_not_a_constant() {
    assert_eq!(extract("const a = '\\uD800';", "a"), None);
    assert_eq!(extract("const a = '\\uD800' + 'x';", "a"), None);
    // A pair split across two literals becomes a well-formed string after concatenation.
    assert_eq!(extract("const a = '\\uD800' + '\\uDC00';", "a"), Some(string("\u{10000}")));
  }

  #[test]
  fn test_template_literal_does_not_cache_object_coercions() {
    assert_eq!(extract("const a = `${/foo/ig}`;", "a"), None);
    assert_eq!(extract("const a = `${{}}`;", "a"), None);
    assert_eq!(extract("const a = `${[1, 2]}`;", "a"), None);
    assert_eq!(extract("const a = `${`${/foo/ig}`}`;", "a"), None);
    // oxc also converts an object operand of a compound expression.
    assert_eq!(extract("const a = `${/foo/ig + ''}`;", "a"), None);
    assert_eq!(extract("const a = /foo/ig + '';", "a"), None);
    assert_eq!(extract("const C = /foo/ig + ''; const a = `${C}`;", "a"), None);
  }

  #[test]
  fn test_template_literal_uses_primitive_values_instead_of_source_spelling() {
    assert_eq!(extract("const a = `${0xffn}`;", "a"), Some(string("255")));
    assert_eq!(extract("const a = `${0b10n}`;", "a"), Some(string("2")));
    assert_eq!(extract("const a = `${-0}`;", "a"), Some(string("0")));
  }
}
