use oxc::{
  allocator::{Allocator, IntoIn as _, TakeIn},
  ast::{
    AstBuilder,
    ast::{
      AssignmentTargetMaybeDefault, AssignmentTargetProperty, BindingPattern, BindingProperty,
    },
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
    let builder = AstBuilder::new(alloc);
    match self.value {
      BindingPattern::AssignmentPattern(assign_pat) => {
        let assign_pat = assign_pat.unbox();
        if self.shorthand {
          let binding_id = assign_pat.left.get_binding_identifier().unwrap();
          AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
            builder.alloc_assignment_target_property_identifier(
              self.span,
              builder.identifier_reference(binding_id.span, binding_id.name),
              Some(assign_pat.right),
            ),
          )
        } else {
          AssignmentTargetProperty::AssignmentTargetPropertyProperty(
            builder.alloc_assignment_target_property_property(
              self.span,
              self.key,
              AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(
                builder.alloc_assignment_target_with_default(
                  assign_pat.span,
                  assign_pat.left.into_assignment_target(alloc),
                  assign_pat.right,
                ),
              ),
              self.computed,
            ),
          )
          .into_in(alloc)
        }
      }
      BindingPattern::BindingIdentifier(ref id) => {
        if self.shorthand {
          AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
            builder.alloc_assignment_target_property_identifier(
              self.span,
              builder.identifier_reference(id.span, id.name),
              None,
            ),
          )
        } else {
          AssignmentTargetProperty::AssignmentTargetPropertyProperty(
            builder.alloc_assignment_target_property_property(
              self.span,
              self.key,
              AssignmentTargetMaybeDefault::from(self.value.into_assignment_target(alloc)),
              self.computed,
            ),
          )
          .into_in(alloc)
        }
      }
      BindingPattern::ArrayPattern(arr_pat) => {
        let mut arr_pat = arr_pat.unbox();
        let rest = arr_pat.rest.take().map(|rest| {
          builder.alloc_assignment_target_rest(
            rest.span,
            rest.unbox().argument.into_assignment_target(alloc),
          )
        });
        let mut elements = builder.vec_with_capacity(arr_pat.elements.len());
        arr_pat.elements.take_in(alloc).into_iter().for_each(|element| {
          elements.push(element.map(|binding_pat| {
            AssignmentTargetMaybeDefault::from(binding_pat.into_assignment_target(alloc))
          }));
        });
        AssignmentTargetProperty::AssignmentTargetPropertyProperty(
          builder.alloc_assignment_target_property_property(
            self.span,
            self.key,
            AssignmentTargetMaybeDefault::ArrayAssignmentTarget(
              builder.alloc_array_assignment_target(arr_pat.span, elements, rest),
            ),
            self.computed,
          ),
        )
        .into_in(alloc)
      }
      BindingPattern::ObjectPattern(obj_pat) => {
        let mut obj_pat = obj_pat.unbox();
        let rest = obj_pat.rest.take().map(|rest| {
          builder.alloc_assignment_target_rest(
            rest.span,
            rest.unbox().argument.into_assignment_target(alloc),
          )
        });
        let mut properties = builder.vec_with_capacity(obj_pat.properties.len());
        obj_pat.properties.take_in(alloc).into_iter().for_each(|property| {
          properties.push(property.into_assignment_target_property(alloc));
        });
        AssignmentTargetProperty::AssignmentTargetPropertyProperty(
          builder.alloc_assignment_target_property_property(
            self.span,
            self.key,
            AssignmentTargetMaybeDefault::ObjectAssignmentTarget(
              builder.alloc_object_assignment_target(obj_pat.span, properties, rest),
            ),
            self.computed,
          ),
        )
        .into_in(alloc)
      }
    }
  }
}
