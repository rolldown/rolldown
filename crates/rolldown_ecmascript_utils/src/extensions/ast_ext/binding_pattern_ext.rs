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
        AssignmentTarget::new_object_assignment_target(SPAN, properties, rest, builder)
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
          elements.push(binding_pat.map(|binding_pat| match binding_pat {
            BindingPattern::AssignmentPattern(assign_pat) => {
              let assign_pat = assign_pat.unbox();
              AssignmentTargetMaybeDefault::new_assignment_target_with_default(
                assign_pat.span,
                assign_pat.left.into_assignment_target(builder),
                assign_pat.right,
                builder,
              )
            }
            _ => AssignmentTargetMaybeDefault::from(binding_pat.into_assignment_target(builder)),
          }));
        });
        AssignmentTarget::new_array_assignment_target(arr_pat.span, elements, rest, builder)
      }
      BindingPattern::AssignmentPattern(_) => {
        unreachable!("`BindingPattern::AssignmentPattern` should be pre-handled in above")
      }
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
      ast::{ArrayExpressionElement, Expression, ObjectPropertyKind, Statement},
      builder::AstBuilder,
    },
    parser::Parser,
    span::SourceType,
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
}
