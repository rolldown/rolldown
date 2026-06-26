use oxc::{
  allocator::TakeIn as _,
  ast::ast::{
    ArrayExpressionElement, AssignmentTarget, AssignmentTargetMaybeDefault, BindingPattern,
    Expression, ObjectPropertyKind, PropertyKind,
  },
  span::SPAN,
};

use crate::AstFactory;

use super::binding_property_ext::BindingPropertyExt as _;

pub trait BindingPatternExt<'ast> {
  fn into_assignment_target(self, ast_factory: &AstFactory<'ast>) -> AssignmentTarget<'ast>;

  fn into_expression(self, ast_factory: &AstFactory<'ast>) -> Expression<'ast>;
}

impl<'ast> BindingPatternExt<'ast> for BindingPattern<'ast> {
  fn into_assignment_target(self, ast_factory: &AstFactory<'ast>) -> AssignmentTarget<'ast> {
    match self {
      // Turn `var a = 1` into `a = 1`
      BindingPattern::BindingIdentifier(id) => AssignmentTarget::AssignmentTargetIdentifier(
        ast_factory.alloc_identifier_reference(id.span, id.name),
      ),
      // Turn `var { a, b = 2 } = ...` to `{a, b = 2} = ...`
      BindingPattern::ObjectPattern(mut obj_pat) => {
        let rest = obj_pat.rest.take().map(|rest| {
          ast_factory.alloc_assignment_target_rest(
            rest.span,
            rest.unbox().argument.into_assignment_target(ast_factory),
          )
        });
        let mut properties = ast_factory.vec_with_capacity(obj_pat.properties.len());
        obj_pat.properties.take_in(ast_factory.allocator).into_iter().for_each(|binding_prop| {
          properties.push(binding_prop.into_assignment_target_property(ast_factory));
        });
        AssignmentTarget::ObjectAssignmentTarget(
          ast_factory.alloc_object_assignment_target(SPAN, properties, rest),
        )
      }
      // Turn `var [a, ,c = 1] = ...` to `[a, ,c = 1] = ...`
      BindingPattern::ArrayPattern(mut arr_pat) => {
        let rest = arr_pat.rest.take().map(|rest| {
          ast_factory.alloc_assignment_target_rest(
            rest.span,
            rest.unbox().argument.into_assignment_target(ast_factory),
          )
        });
        let mut elements = ast_factory.vec_with_capacity(arr_pat.elements.len());
        arr_pat.elements.take_in(ast_factory.allocator).into_iter().for_each(|binding_pat| {
          elements.push(binding_pat.map(|binding_pat| match binding_pat {
            BindingPattern::AssignmentPattern(assign_pat) => {
              let assign_pat = assign_pat.unbox();
              AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(
                ast_factory.alloc_assignment_target_with_default(
                  assign_pat.span,
                  assign_pat.left.into_assignment_target(ast_factory),
                  assign_pat.right,
                ),
              )
            }
            _ => {
              AssignmentTargetMaybeDefault::from(binding_pat.into_assignment_target(ast_factory))
            }
          }));
        });
        AssignmentTarget::ArrayAssignmentTarget(ast_factory.alloc_array_assignment_target(
          arr_pat.span,
          elements,
          rest,
        ))
      }
      BindingPattern::AssignmentPattern(_) => {
        unreachable!("`BindingPattern::AssignmentPattern` should be pre-handled in above")
      }
    }
  }

  fn into_expression(self, ast_factory: &AstFactory<'ast>) -> Expression<'ast> {
    match self {
      BindingPattern::BindingIdentifier(id) => ast_factory.expression_identifier(SPAN, id.name),
      BindingPattern::ObjectPattern(mut obj_pat) => {
        let capacity = obj_pat.properties.len() + usize::from(obj_pat.rest.is_some());
        let mut properties = ast_factory.vec_with_capacity(capacity);
        obj_pat.properties.take_in(ast_factory.allocator).into_iter().for_each(|binding_prop| {
          properties.push(ObjectPropertyKind::ObjectProperty(ast_factory.alloc_object_property(
            SPAN,
            PropertyKind::Init,
            binding_prop.key,
            binding_prop.value.into_expression(ast_factory),
            false,
            binding_prop.shorthand,
            binding_prop.computed,
          )));
        });
        if let Some(rest) = obj_pat.rest.take() {
          let BindingPattern::BindingIdentifier(ref id) = rest.argument else {
            unreachable!("The rest element should be `BindingIdentifier`")
          };
          properties.push(ObjectPropertyKind::SpreadProperty(
            ast_factory
              .alloc_spread_element(SPAN, ast_factory.expression_identifier(SPAN, id.name)),
          ));
        }
        Expression::ObjectExpression(ast_factory.alloc_object_expression(SPAN, properties))
      }
      BindingPattern::ArrayPattern(mut arg_pat) => {
        let capacity = arg_pat.elements.len() + usize::from(arg_pat.rest.is_some());
        let mut elements = ast_factory.vec_with_capacity(capacity);
        arg_pat.elements.take_in(ast_factory.allocator).into_iter().for_each(|binding_pat| {
          elements.push(binding_pat.map_or(
            ArrayExpressionElement::Elision(ast_factory.alloc_elision(SPAN)),
            |binding_pat| ArrayExpressionElement::from(binding_pat.into_expression(ast_factory)),
          ));
        });
        if let Some(rest) = arg_pat.rest.take() {
          let BindingPattern::BindingIdentifier(ref id) = rest.argument else {
            unreachable!("The rest element should be `BindingIdentifier`")
          };
          elements.push(ArrayExpressionElement::SpreadElement(
            ast_factory
              .alloc_spread_element(SPAN, ast_factory.expression_identifier(SPAN, id.name)),
          ));
        }
        Expression::ArrayExpression(ast_factory.alloc_array_expression(SPAN, elements))
      }
      BindingPattern::AssignmentPattern(mut assign_pat) => {
        assign_pat.left.take_in(ast_factory.allocator).into_expression(ast_factory)
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use oxc::{
    allocator::{Allocator, CloneIn},
    ast::ast::{ArrayExpressionElement, Expression, ObjectPropertyKind, Statement},
    parser::Parser,
    span::SourceType,
  };

  use crate::{AstFactory, BindingPatternExt as _};

  /// Round-trips the first declarator's binding pattern back to an expression via
  /// `into_expression`. `source` must be a single `const <pattern> = x;` statement.
  fn pattern_into_expression<'a>(allocator: &'a Allocator, source: &'a str) -> Expression<'a> {
    let program = Parser::new(allocator, source, SourceType::default()).parse().program;
    let Some(Statement::VariableDeclaration(decl)) = program.body.first() else {
      unreachable!("expected a variable declaration")
    };
    let pattern = decl.declarations[0].id.clone_in(allocator);
    pattern.into_expression(&AstFactory::new(allocator))
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
}
