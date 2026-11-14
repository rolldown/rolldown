use oxc::allocator::GetAllocator;
use oxc::{
  allocator::TakeIn as _,
  ast::{
    ast::{
      ArrayExpressionElement, AssignmentTarget, AssignmentTargetMaybeDefault, AssignmentTargetRest,
      BindingPattern, Expression, ObjectPropertyKind, PropertyKind,
    },
    builder::GetAstBuilder,
  },
  span::SPAN,
};

use super::binding_property_ext::BindingPropertyExt as _;

pub trait BindingPatternExt<'ast> {
  fn into_assignment_target<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    self,
    builder: &B,
  ) -> AssignmentTarget<'ast>;

  /// Converts a pattern in a position that allows a default: an array element, or a property
  /// value that is not shorthand. The spec calls both positions `BindingElement`.
  ///
  /// Use this method, not `into_assignment_target`, for these positions. `AssignmentTarget` cannot
  /// hold a default, but `AssignmentTargetMaybeDefault` can.
  fn into_assignment_target_maybe_default<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    self,
    builder: &B,
  ) -> AssignmentTargetMaybeDefault<'ast>;

  fn into_expression<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    self,
    builder: &B,
  ) -> Expression<'ast>;
}

impl<'ast> BindingPatternExt<'ast> for BindingPattern<'ast> {
  fn into_assignment_target<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    self,
    builder: &B,
  ) -> AssignmentTarget<'ast> {
    match self {
      // Turn `var a = 1` into `a = 1`
      BindingPattern::BindingIdentifier(id) => {
        AssignmentTarget::new_assignment_target_identifier(id.span, id.name, builder)
      }
      // Turn `var { a, b = 2 } = ...` to `{a, b = 2} = ...`
      BindingPattern::ObjectPattern(mut obj_pat) => {
        let rest = obj_pat.rest.take().map(|rest| {
          AssignmentTargetRest::boxed(
            rest.span,
            rest.unbox().argument.into_assignment_target(builder),
            builder,
          )
        });
        let mut properties =
          oxc::allocator::Vec::with_capacity_in(obj_pat.properties.len(), builder);
        obj_pat.properties.take_in(&builder.allocator()).into_iter().for_each(|binding_prop| {
          properties.push(binding_prop.into_assignment_target_property(builder));
        });
        AssignmentTarget::new_object_assignment_target(obj_pat.span, properties, rest, builder)
      }
      // Turn `var [a, ,c = 1] = ...` to `[a, ,c = 1] = ...`
      BindingPattern::ArrayPattern(mut arr_pat) => {
        let rest = arr_pat.rest.take().map(|rest| {
          AssignmentTargetRest::boxed(
            rest.span,
            rest.unbox().argument.into_assignment_target(builder),
            builder,
          )
        });
        let mut elements = oxc::allocator::Vec::with_capacity_in(arr_pat.elements.len(), builder);
        arr_pat.elements.take_in(&builder.allocator()).into_iter().for_each(|binding_pat| {
          elements.push(binding_pat.map(|pat| pat.into_assignment_target_maybe_default(builder)));
        });
        AssignmentTarget::new_array_assignment_target(arr_pat.span, elements, rest, builder)
      }
      // `a = 1` in `[a = 1]` or `{ k: a = 1 }`. Only an array element or a property value can
      // have a default, and `into_assignment_target_maybe_default` converts those. A declarator
      // id (`[a]` in `var [a] = x`) or a rest argument (`...a`) cannot have a default.
      BindingPattern::AssignmentPattern(_) => {
        unreachable!(
          "`AssignmentTarget` cannot hold a default. Convert array elements and property values with `into_assignment_target_maybe_default`"
        )
      }
    }
  }

  fn into_assignment_target_maybe_default<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    self,
    builder: &B,
  ) -> AssignmentTargetMaybeDefault<'ast> {
    match self {
      // Turn `[a = 1]` or `{ k: a = 1 }` into the same assignment element with its default
      BindingPattern::AssignmentPattern(assign_pat) => {
        let assign_pat = assign_pat.unbox();
        AssignmentTargetMaybeDefault::new_assignment_target_with_default(
          assign_pat.span,
          assign_pat.left.into_assignment_target(builder),
          assign_pat.right,
          builder,
        )
      }
      pattern => AssignmentTargetMaybeDefault::from(pattern.into_assignment_target(builder)),
    }
  }

  fn into_expression<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    self,
    builder: &B,
  ) -> Expression<'ast> {
    match self {
      BindingPattern::BindingIdentifier(id) => Expression::new_identifier(SPAN, id.name, builder),
      BindingPattern::ObjectPattern(mut obj_pat) => {
        let capacity = obj_pat.properties.len() + usize::from(obj_pat.rest.is_some());
        let mut properties = oxc::allocator::Vec::with_capacity_in(capacity, builder);
        obj_pat.properties.take_in(&builder.allocator()).into_iter().for_each(|binding_prop| {
          properties.push(ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            binding_prop.key,
            binding_prop.value.into_expression(builder),
            false,
            binding_prop.shorthand,
            binding_prop.computed,
            builder,
          ));
        });
        if let Some(rest) = obj_pat.rest.take() {
          properties.push(ObjectPropertyKind::new_spread_property(
            SPAN,
            rest.unbox().argument.into_expression(builder),
            builder,
          ));
        }
        Expression::new_object_expression(SPAN, properties, builder)
      }
      BindingPattern::ArrayPattern(mut arg_pat) => {
        let capacity = arg_pat.elements.len() + usize::from(arg_pat.rest.is_some());
        let mut elements = oxc::allocator::Vec::with_capacity_in(capacity, builder);
        arg_pat.elements.take_in(&builder.allocator()).into_iter().for_each(|binding_pat| {
          elements.push(
            binding_pat.map_or(ArrayExpressionElement::new_elision(SPAN, builder), |binding_pat| {
              ArrayExpressionElement::from(binding_pat.into_expression(builder))
            }),
          );
        });
        if let Some(rest) = arg_pat.rest.take() {
          elements.push(ArrayExpressionElement::new_spread_element(
            SPAN,
            rest.unbox().argument.into_expression(builder),
            builder,
          ));
        }
        Expression::new_array_expression(SPAN, elements, builder)
      }
      BindingPattern::AssignmentPattern(mut assign_pat) => {
        assign_pat.left.take_in(&builder.allocator()).into_expression(builder)
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use oxc::{
    allocator::{Allocator, CloneIn},
    ast::{
      ast::{
        ArrayExpressionElement, AssignmentOperator, Expression, ObjectPropertyKind, Statement,
      },
      builder::AstBuilder,
    },
    codegen::Codegen,
    parser::Parser,
    span::{SPAN, SourceType},
  };

  use crate::BindingPatternExt as _;

  /// Round-trips the first declarator's binding pattern back to an expression via
  /// `into_expression`. `source` must be a single `const <pattern> = x;` statement.
  fn pattern_into_expression<'a>(allocator: &'a Allocator, source: &'a str) -> Expression<'a> {
    let program = Parser::new(allocator, source, SourceType::default()).parse().program;
    let Some(Statement::VariableDeclaration(decl)) = program.body.first() else {
      unreachable!("expected a variable declaration")
    };
    let pattern = decl.declarations[0].id.clone_in(allocator);
    pattern.into_expression(&AstBuilder::new(allocator))
  }

  #[test]
  fn object_rest_round_trips_to_spread_property() {
    let allocator = Allocator::default();
    let Expression::ObjectExpression(obj) =
      pattern_into_expression(&allocator, "const { a, ...rest } = x;")
    else {
      unreachable!("expected an object expression")
    };
    // `{ a, ...rest }` must round-trip with a spread, not a `rest` shorthand property.
    assert!(matches!(obj.properties.last(), Some(ObjectPropertyKind::SpreadProperty(_))));
  }

  #[test]
  fn array_rest_round_trips_to_spread_element() {
    let allocator = Allocator::default();
    let Expression::ArrayExpression(arr) =
      pattern_into_expression(&allocator, "const [a, ...rest] = x;")
    else {
      unreachable!("expected an array expression")
    };
    // `[a, ...rest]` must round-trip with a spread, not a plain element.
    assert!(matches!(arr.elements.last(), Some(ArrayExpressionElement::SpreadElement(_))));
  }

  #[test]
  fn array_nested_rest_round_trips_to_spread() {
    let allocator = Allocator::default();
    let Expression::ArrayExpression(arr) =
      pattern_into_expression(&allocator, "const [a, ...[b, c]] = x;")
    else {
      unreachable!("expected an array expression")
    };
    // A nested rest pattern (`...[b, c]`) is legal JS; it must round-trip as a spread of
    // the rebuilt inner pattern rather than panicking on the non-identifier argument.
    let Some(ArrayExpressionElement::SpreadElement(spread)) = arr.elements.last() else {
      unreachable!("expected a spread element")
    };
    assert!(matches!(spread.argument, Expression::ArrayExpression(_)));
  }

  /// Changes `const <pattern> = <init>;` into `<pattern> = <init>;` with `into_assignment_target`,
  /// as the finalizer does for a wrapped module, and prints the result.
  fn declaration_into_assignment(source: &str) -> String {
    let allocator = Allocator::default();
    let builder = AstBuilder::new(&allocator);
    let mut program = Parser::new(&allocator, source, SourceType::default()).parse().program;
    let Some(Statement::VariableDeclaration(decl)) = program.body.first() else {
      unreachable!("expected a variable declaration")
    };
    let declarator = &decl.declarations[0];
    let left = declarator.id.clone_in(&allocator).into_assignment_target(&builder);
    let right = declarator.init.clone_in(&allocator).expect("expected an initializer");
    let assignment = Expression::new_assignment_expression(
      SPAN,
      AssignmentOperator::Assign,
      left,
      right,
      &builder,
    );
    program.body.clear();
    program.body.push(Statement::new_expression_statement(SPAN, assignment, &builder));
    Codegen::new().build(&program).code
  }

  #[test]
  fn assignment_target_keeps_every_default() {
    let cases = [
      // A default on an array element in a property value (#11552)
      ("const { b: [y = 1] } = x;", "({b: [y = 1]} = x);\n"),
      ("const { b: [, y = 1] } = x;", "({b: [, y = 1]} = x);\n"),
      ("const [{ b: [y = 1] }] = x;", "[{b: [y = 1]}] = x;\n"),
      ("const { [k]: [y = 1] } = x;", "({[k]: [y = 1]} = x);\n"),
      ("const { b: [y = 1] = [] } = x;", "({b: [y = 1] = []} = x);\n"),
      // Each position that allows a default, with a default
      (
        "const { a = 1, b: c = 2, d: { e = 3 }, f: [g = 4, ...h], ...i } = x;",
        "({a = 1, b: c = 2, d: {e = 3}, f: [g = 4, ...h], ...i} = x);\n",
      ),
      (
        "const [a = 1, [b = 2], { c = 3 }, ...[d = 4]] = x;",
        "[a = 1, [b = 2], {c = 3}, ...[d = 4]] = x;\n",
      ),
    ];
    for (source, expected) in cases {
      assert_eq!(declaration_into_assignment(source), expected, "source: {source}");
    }
  }
}
