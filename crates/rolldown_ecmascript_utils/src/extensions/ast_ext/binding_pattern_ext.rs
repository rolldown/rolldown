use oxc::{
  allocator::{Allocator, TakeIn as _},
  ast::{
    AstBuilder,
    ast::{
      ArrayExpressionElement, AssignmentTarget, AssignmentTargetMaybeDefault, BindingPattern,
      Expression, ObjectPropertyKind, PropertyKind,
    },
  },
  span::SPAN,
};

use crate::AstSnippet;

use super::binding_property_ext::BindingPropertyExt as _;

pub trait BindingPatternExt<'ast> {
  fn into_assignment_target(self, alloc: &'ast Allocator) -> AssignmentTarget<'ast>;

  fn into_expression(self, snippet: &AstSnippet<'ast>) -> Expression<'ast>;
}

impl<'ast> BindingPatternExt<'ast> for BindingPattern<'ast> {
  fn into_assignment_target(self, alloc: &'ast Allocator) -> AssignmentTarget<'ast> {
    match self {
      // Turn `var a = 1` into `a = 1`
      BindingPattern::BindingIdentifier(id) => {
        AstSnippet::new(alloc).simple_id_assignment_target(&id.name, id.span)
      }
      // Turn `var { a, b = 2 } = ...` to `{a, b = 2} = ...`
      BindingPattern::ObjectPattern(mut obj_pat) => {
        let builder = AstBuilder::new(alloc);
        let rest = obj_pat.rest.take().map(|rest| {
          builder.alloc_assignment_target_rest(
            rest.span,
            rest.unbox().argument.into_assignment_target(alloc),
          )
        });
        let mut properties = builder.vec_with_capacity(obj_pat.properties.len());
        obj_pat.properties.take_in(alloc).into_iter().for_each(|binding_prop| {
          properties.push(binding_prop.into_assignment_target_property(alloc));
        });
        AssignmentTarget::ObjectAssignmentTarget(
          builder.alloc_object_assignment_target(SPAN, properties, rest),
        )
      }
      // Turn `var [a, ,c = 1] = ...` to `[a, ,c = 1] = ...`
      BindingPattern::ArrayPattern(mut arr_pat) => {
        let builder = AstBuilder::new(alloc);
        let rest = arr_pat.rest.take().map(|rest| {
          builder.alloc_assignment_target_rest(
            rest.span,
            rest.unbox().argument.into_assignment_target(alloc),
          )
        });
        let mut elements = builder.vec_with_capacity(arr_pat.elements.len());
        arr_pat.elements.take_in(alloc).into_iter().for_each(|binding_pat| {
          elements.push(binding_pat.map(|binding_pat| match binding_pat {
            BindingPattern::AssignmentPattern(assign_pat) => {
              let assign_pat = assign_pat.unbox();
              AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(
                builder.alloc_assignment_target_with_default(
                  assign_pat.span,
                  assign_pat.left.into_assignment_target(alloc),
                  assign_pat.right,
                ),
              )
            }
            _ => AssignmentTargetMaybeDefault::from(binding_pat.into_assignment_target(alloc)),
          }));
        });
        AssignmentTarget::ArrayAssignmentTarget(builder.alloc_array_assignment_target(
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

  fn into_expression(self, snippet: &AstSnippet<'ast>) -> Expression<'ast> {
    match self {
      BindingPattern::BindingIdentifier(id) => snippet.builder.expression_identifier(SPAN, id.name),
      BindingPattern::ObjectPattern(mut obj_pat) => {
        let capacity = obj_pat.properties.len() + usize::from(obj_pat.rest.is_some());
        let mut properties = snippet.builder.vec_with_capacity(capacity);
        obj_pat.properties.take_in(snippet.alloc()).into_iter().for_each(|binding_prop| {
          properties.push(ObjectPropertyKind::ObjectProperty(
            snippet.builder.alloc_object_property(
              SPAN,
              PropertyKind::Init,
              binding_prop.key,
              binding_prop.value.into_expression(snippet),
              false,
              binding_prop.shorthand,
              binding_prop.computed,
            ),
          ));
        });
        if let Some(rest) = obj_pat.rest.take() {
          let BindingPattern::BindingIdentifier(ref id) = rest.argument else {
            unreachable!("The rest element should be `BindingIdentifier`")
          };
          properties.push(ObjectPropertyKind::ObjectProperty(
            snippet.builder.alloc_object_property(
              SPAN,
              PropertyKind::Init,
              snippet.builder.property_key_static_identifier(SPAN, id.name),
              snippet.builder.expression_identifier(SPAN, id.name),
              false,
              true,
              false,
            ),
          ));
        }
        Expression::ObjectExpression(snippet.builder.alloc_object_expression(SPAN, properties))
      }
      BindingPattern::ArrayPattern(mut arg_pat) => {
        let capacity = arg_pat.elements.len() + usize::from(arg_pat.rest.is_some());
        let mut elements = snippet.builder.vec_with_capacity(capacity);
        arg_pat.elements.take_in(snippet.alloc()).into_iter().for_each(|binding_pat| {
          elements.push(binding_pat.map_or(
            ArrayExpressionElement::Elision(snippet.builder.alloc_elision(SPAN)),
            |binding_pat| ArrayExpressionElement::from(binding_pat.into_expression(snippet)),
          ));
        });
        if let Some(rest) = arg_pat.rest.take() {
          let BindingPattern::BindingIdentifier(ref id) = rest.argument else {
            unreachable!("The rest element should be `BindingIdentifier`")
          };
          elements.push(ArrayExpressionElement::Identifier(
            snippet.builder.alloc_identifier_reference(SPAN, id.name),
          ));
        }
        Expression::ArrayExpression(snippet.builder.alloc_array_expression(SPAN, elements))
      }
      BindingPattern::AssignmentPattern(mut assign_pat) => {
        assign_pat.left.take_in(snippet.alloc()).into_expression(snippet)
      }
    }
  }
}
