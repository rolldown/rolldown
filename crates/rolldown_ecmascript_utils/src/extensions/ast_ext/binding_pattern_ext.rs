use oxc::{
  allocator::{Allocator, Box, Dummy as _, IntoIn as _, TakeIn as _},
  ast::ast::{
    ArrayAssignmentTarget, ArrayExpressionElement, AssignmentTarget, AssignmentTargetMaybeDefault,
    AssignmentTargetRest, AssignmentTargetWithDefault, BindingIdentifier, BindingPattern,
    BindingPatternKind, Expression, ObjectAssignmentTarget, ObjectPropertyKind, PropertyKind,
  },
  span::SPAN,
};
use smallvec::SmallVec;

use crate::AstSnippet;

use super::binding_property_ext::BindingPropertyExt as _;

pub trait BindingPatternExt<'ast> {
  fn binding_identifiers(&self) -> smallvec::SmallVec<[&Box<BindingIdentifier<'ast>>; 1]>;

  fn into_assignment_target(self, alloc: &'ast Allocator) -> AssignmentTarget<'ast>;

  fn into_expression(self, snippet: &AstSnippet<'ast>) -> Expression<'ast>;
}

impl<'ast> BindingPatternExt<'ast> for BindingPattern<'ast> {
  fn binding_identifiers(&self) -> smallvec::SmallVec<[&Box<BindingIdentifier<'ast>>; 1]> {
    let mut stack = vec![&self.kind];
    let mut ret = SmallVec::default();
    while let Some(binding_kind) = stack.pop() {
      match binding_kind {
        BindingPatternKind::BindingIdentifier(id) => {
          ret.push(id);
        }
        BindingPatternKind::ArrayPattern(arr_pat) => {
          stack.extend(arr_pat.elements.iter().flatten().map(|pat| &pat.kind).rev());
        }
        BindingPatternKind::ObjectPattern(obj_pat) => {
          if let Some(obj_pat) = &obj_pat.rest {
            stack.push(&obj_pat.argument.kind);
          }
          stack.extend(obj_pat.properties.iter().map(|prop| &prop.value.kind).rev());
        }
        //
        BindingPatternKind::AssignmentPattern(assign_pat) => {
          stack.push(&assign_pat.left.kind);
        }
      }
    }
    ret
  }

  fn into_assignment_target(mut self, alloc: &'ast Allocator) -> AssignmentTarget<'ast> {
    match &mut self.kind {
      // Turn `var a = 1` into `a = 1`
      BindingPatternKind::BindingIdentifier(id) => {
        AstSnippet::new(alloc).simple_id_assignment_target(&id.name, id.span)
      }
      // Turn `var { a, b = 2 } = ...` to `{a, b = 2} = ...`
      BindingPatternKind::ObjectPattern(obj_pat) => {
        let mut obj_target = ObjectAssignmentTarget {
          rest: obj_pat.rest.take().map(|rest| AssignmentTargetRest {
            span: rest.span,
            target: rest.unbox().argument.into_assignment_target(alloc),
          }),
          ..ObjectAssignmentTarget::dummy(alloc)
        };
        obj_pat.properties.take_in(alloc).into_iter().for_each(|binding_prop| {
          obj_target.properties.push(binding_prop.into_assignment_target_property(alloc));
        });
        AssignmentTarget::ObjectAssignmentTarget(obj_target.into_in(alloc))
      }
      // Turn `var [a, ,c = 1] = ...` to `[a, ,c = 1] = ...`
      BindingPatternKind::ArrayPattern(arr_pat) => {
        let mut arr_target = ArrayAssignmentTarget {
          span: arr_pat.span,
          rest: arr_pat.rest.take().map(|rest| AssignmentTargetRest {
            span: rest.span,
            target: rest.unbox().argument.into_assignment_target(alloc),
          }),
          elements: oxc::allocator::Vec::with_capacity_in(arr_pat.elements.len(), alloc),
        };
        arr_pat.elements.take_in(alloc).into_iter().for_each(|binding_pat| {
          arr_target.elements.push(binding_pat.map(|binding_pat| match binding_pat.kind {
            BindingPatternKind::AssignmentPattern(assign_pat) => {
              let assign_pat = assign_pat.unbox();
              AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(
                AssignmentTargetWithDefault {
                  span: assign_pat.span,
                  init: assign_pat.right,
                  binding: assign_pat.left.into_assignment_target(alloc),
                }
                .into_in(alloc),
              )
            }
            _ => AssignmentTargetMaybeDefault::from(binding_pat.into_assignment_target(alloc)),
          }));
        });
        AssignmentTarget::ArrayAssignmentTarget(arr_target.into_in(alloc))
      }
      BindingPatternKind::AssignmentPattern(_) => {
        unreachable!("`BindingPatternKind::AssignmentPattern` should be pre-handled in above")
      }
    }
  }

  fn into_expression(self, snippet: &AstSnippet<'ast>) -> Expression<'ast> {
    match self.kind {
      BindingPatternKind::BindingIdentifier(id) => {
        snippet.builder.expression_identifier(SPAN, id.name)
      }
      BindingPatternKind::ObjectPattern(mut obj_pat) => {
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
          let BindingPatternKind::BindingIdentifier(ref id) = rest.argument.kind else {
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
      BindingPatternKind::ArrayPattern(mut arg_pat) => {
        let capacity = arg_pat.elements.len() + usize::from(arg_pat.rest.is_some());
        let mut elements = snippet.builder.vec_with_capacity(capacity);
        arg_pat.elements.take_in(snippet.alloc()).into_iter().for_each(|binding_pat| {
          elements.push(binding_pat.map_or(
            ArrayExpressionElement::Elision(snippet.builder.elision(SPAN)),
            |binding_pat| ArrayExpressionElement::from(binding_pat.into_expression(snippet)),
          ));
        });
        if let Some(rest) = arg_pat.rest.take() {
          let BindingPatternKind::BindingIdentifier(ref id) = rest.argument.kind else {
            unreachable!("The rest element should be `BindingIdentifier`")
          };
          elements.push(ArrayExpressionElement::Identifier(
            snippet.builder.alloc_identifier_reference(SPAN, id.name),
          ));
        }
        Expression::ArrayExpression(snippet.builder.alloc_array_expression(SPAN, elements))
      }
      BindingPatternKind::AssignmentPattern(mut assign_pat) => {
        assign_pat.left.take_in(snippet.alloc()).into_expression(snippet)
      }
    }
  }
}
