use oxc::allocator::GetAllocator;
use oxc::{
  allocator::{IntoIn as _, TakeIn},
  ast::ast::{
    ArrayAssignmentTarget, AssignmentTargetMaybeDefault, AssignmentTargetProperty,
    AssignmentTargetPropertyIdentifier, AssignmentTargetPropertyProperty, AssignmentTargetRest,
    AssignmentTargetWithDefault, BindingPattern, BindingProperty, IdentifierReference,
    ObjectAssignmentTarget,
  },
};

use crate::{AstFactory, BindingPatternExt as _};

pub trait BindingPropertyExt<'ast> {
  fn into_assignment_target_property(
    self,
    ast_factory: &AstFactory<'ast>,
  ) -> AssignmentTargetProperty<'ast>;
}

impl<'ast> BindingPropertyExt<'ast> for BindingProperty<'ast> {
  fn into_assignment_target_property(
    self,
    ast_factory: &AstFactory<'ast>,
  ) -> AssignmentTargetProperty<'ast> {
    match self.value {
      BindingPattern::AssignmentPattern(assign_pat) => {
        let assign_pat = assign_pat.unbox();
        if self.shorthand {
          let binding_id = assign_pat.left.get_binding_identifier().unwrap();
          AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
            AssignmentTargetPropertyIdentifier::boxed(
              self.span,
              IdentifierReference::new(binding_id.span, binding_id.name, ast_factory),
              Some(assign_pat.right),
              ast_factory,
            ),
          )
        } else {
          AssignmentTargetProperty::AssignmentTargetPropertyProperty(
            AssignmentTargetPropertyProperty::boxed(
              self.span,
              self.key,
              AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(
                AssignmentTargetWithDefault::boxed(
                  assign_pat.span,
                  assign_pat.left.into_assignment_target(ast_factory),
                  assign_pat.right,
                  ast_factory,
                ),
              ),
              self.computed,
              ast_factory,
            ),
          )
          .into_in(ast_factory.allocator())
        }
      }
      BindingPattern::BindingIdentifier(ref id) => {
        if self.shorthand {
          AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
            AssignmentTargetPropertyIdentifier::boxed(
              self.span,
              IdentifierReference::new(id.span, id.name, ast_factory),
              None,
              ast_factory,
            ),
          )
        } else {
          AssignmentTargetProperty::AssignmentTargetPropertyProperty(
            AssignmentTargetPropertyProperty::boxed(
              self.span,
              self.key,
              AssignmentTargetMaybeDefault::from(self.value.into_assignment_target(ast_factory)),
              self.computed,
              ast_factory,
            ),
          )
          .into_in(ast_factory.allocator())
        }
      }
      BindingPattern::ArrayPattern(arr_pat) => {
        let mut arr_pat = arr_pat.unbox();
        let rest = arr_pat.rest.take().map(|rest| {
          AssignmentTargetRest::boxed(
            rest.span,
            rest.unbox().argument.into_assignment_target(ast_factory),
            ast_factory,
          )
        });
        let mut elements =
          oxc::allocator::Vec::with_capacity_in(arr_pat.elements.len(), ast_factory);
        arr_pat.elements.take_in(&ast_factory.allocator()).into_iter().for_each(|element| {
          elements.push(element.map(|binding_pat| {
            AssignmentTargetMaybeDefault::from(binding_pat.into_assignment_target(ast_factory))
          }));
        });
        AssignmentTargetProperty::AssignmentTargetPropertyProperty(
          AssignmentTargetPropertyProperty::boxed(
            self.span,
            self.key,
            AssignmentTargetMaybeDefault::ArrayAssignmentTarget(ArrayAssignmentTarget::boxed(
              arr_pat.span,
              elements,
              rest,
              ast_factory,
            )),
            self.computed,
            ast_factory,
          ),
        )
        .into_in(ast_factory.allocator())
      }
      BindingPattern::ObjectPattern(obj_pat) => {
        let mut obj_pat = obj_pat.unbox();
        let rest = obj_pat.rest.take().map(|rest| {
          AssignmentTargetRest::boxed(
            rest.span,
            rest.unbox().argument.into_assignment_target(ast_factory),
            ast_factory,
          )
        });
        let mut properties =
          oxc::allocator::Vec::with_capacity_in(obj_pat.properties.len(), ast_factory);
        obj_pat.properties.take_in(&ast_factory.allocator()).into_iter().for_each(|property| {
          properties.push(property.into_assignment_target_property(ast_factory));
        });
        AssignmentTargetProperty::AssignmentTargetPropertyProperty(
          AssignmentTargetPropertyProperty::boxed(
            self.span,
            self.key,
            AssignmentTargetMaybeDefault::ObjectAssignmentTarget(ObjectAssignmentTarget::boxed(
              obj_pat.span,
              properties,
              rest,
              ast_factory,
            )),
            self.computed,
            ast_factory,
          ),
        )
        .into_in(ast_factory.allocator())
      }
    }
  }
}
