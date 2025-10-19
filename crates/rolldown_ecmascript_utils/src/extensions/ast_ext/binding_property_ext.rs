use oxc::{
  allocator::{Allocator, Box, Dummy as _, IntoIn as _, TakeIn},
  ast::ast::{
    ArrayAssignmentTarget, AssignmentTargetMaybeDefault, AssignmentTargetProperty,
    AssignmentTargetPropertyIdentifier, AssignmentTargetPropertyProperty, AssignmentTargetRest,
    AssignmentTargetWithDefault, BindingPatternKind, BindingProperty, IdentifierReference,
    ObjectAssignmentTarget,
  },
};

use crate::BindingPatternExt as _;

pub trait BindingPropertyExt<'ast> {
  fn into_assignment_target_property(
    self,
    alloc: &'ast Allocator,
  ) -> AssignmentTargetProperty<'ast>;
}

impl<'ast> BindingPropertyExt<'ast> for BindingProperty<'ast> {
  fn into_assignment_target_property(
    self,
    alloc: &'ast Allocator,
  ) -> AssignmentTargetProperty<'ast> {
    match self.value.kind {
      BindingPatternKind::AssignmentPattern(assign_pat) => {
        let assign_pat = assign_pat.unbox();
        if self.shorthand {
          let binding_id = assign_pat.left.get_binding_identifier().unwrap();
          AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
            AssignmentTargetPropertyIdentifier {
              span: self.span,
              init: Some(assign_pat.right),
              binding: IdentifierReference {
                name: binding_id.name,
                span: binding_id.span,
                ..IdentifierReference::dummy(alloc)
              },
            }
            .into_in(alloc),
          )
        } else {
          AssignmentTargetProperty::AssignmentTargetPropertyProperty(
            AssignmentTargetPropertyProperty {
              name: self.key,
              span: self.span,
              binding: AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(
                AssignmentTargetWithDefault {
                  span: assign_pat.span,
                  init: assign_pat.right,
                  binding: assign_pat.left.into_assignment_target(alloc),
                }
                .into_in(alloc),
              ),
              ..AssignmentTargetPropertyProperty::dummy(alloc)
            }
            .into_in(alloc),
          )
          .into_in(alloc)
        }
      }
      BindingPatternKind::BindingIdentifier(ref id) => {
        if self.shorthand {
          AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
            AssignmentTargetPropertyIdentifier {
              init: None,
              span: self.span,
              binding: IdentifierReference {
                name: id.name,
                span: id.span,
                ..IdentifierReference::dummy(alloc)
              },
            }
            .into_in(alloc),
          )
        } else {
          AssignmentTargetProperty::AssignmentTargetPropertyProperty(
            AssignmentTargetPropertyProperty {
              name: self.key,
              span: self.span,
              binding: AssignmentTargetMaybeDefault::from(self.value.into_assignment_target(alloc)),
              ..AssignmentTargetPropertyProperty::dummy(alloc)
            }
            .into_in(alloc),
          )
          .into_in(alloc)
        }
      }
      BindingPatternKind::ArrayPattern(arr_pat) => {
        let mut arr_pat = arr_pat.unbox();
        let mut elements = oxc::allocator::Vec::with_capacity_in(arr_pat.elements.len(), alloc);
        arr_pat.elements.take_in(alloc).into_iter().for_each(|element| {
          elements.push(element.map(|binding_pat| {
            AssignmentTargetMaybeDefault::from(binding_pat.into_assignment_target(alloc))
          }));
        });
        AssignmentTargetProperty::AssignmentTargetPropertyProperty(
          AssignmentTargetPropertyProperty {
            name: self.key,
            span: self.span,
            binding: AssignmentTargetMaybeDefault::ArrayAssignmentTarget(
              ArrayAssignmentTarget {
                elements,
                span: arr_pat.span,
                rest: arr_pat.rest.map(|rest| {
                  Box::new_in(
                    AssignmentTargetRest {
                      span: rest.span,
                      target: rest.unbox().argument.into_assignment_target(alloc),
                    },
                    alloc,
                  )
                }),
              }
              .into_in(alloc),
            ),
            ..AssignmentTargetPropertyProperty::dummy(alloc)
          }
          .into_in(alloc),
        )
        .into_in(alloc)
      }
      BindingPatternKind::ObjectPattern(obj_pat) => {
        let mut obj_pat = obj_pat.unbox();
        let mut properties = oxc::allocator::Vec::with_capacity_in(obj_pat.properties.len(), alloc);
        obj_pat.properties.take_in(alloc).into_iter().for_each(|property| {
          properties.push(property.into_assignment_target_property(alloc));
        });
        AssignmentTargetProperty::AssignmentTargetPropertyProperty(
          AssignmentTargetPropertyProperty {
            name: self.key,
            span: self.span,
            binding: AssignmentTargetMaybeDefault::ObjectAssignmentTarget(
              ObjectAssignmentTarget {
                properties,
                span: obj_pat.span,
                rest: obj_pat.rest.map(|rest| {
                  Box::new_in(
                    AssignmentTargetRest {
                      span: rest.span,
                      target: rest.unbox().argument.into_assignment_target(alloc),
                    },
                    alloc,
                  )
                }),
              }
              .into_in(alloc),
            ),
            ..AssignmentTargetPropertyProperty::dummy(alloc)
          }
          .into_in(alloc),
        )
        .into_in(alloc)
      }
    }
  }
}
