use oxc::allocator::GetAllocator;
use oxc::{
  allocator::IntoIn as _,
  ast::{
    ast::{AssignmentTargetProperty, BindingPattern, BindingProperty, IdentifierReference},
    builder::GetAstBuilder,
  },
};

use crate::BindingPatternExt as _;

pub trait BindingPropertyExt<'ast> {
  fn into_assignment_target_property<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    self,
    builder: &B,
  ) -> AssignmentTargetProperty<'ast>;
}

impl<'ast> BindingPropertyExt<'ast> for BindingProperty<'ast> {
  fn into_assignment_target_property<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    self,
    builder: &B,
  ) -> AssignmentTargetProperty<'ast> {
    match self.value {
      // `{ a = 1 }`: a shorthand property with a default
      BindingPattern::AssignmentPattern(assign_pat) if self.shorthand => {
        let assign_pat = assign_pat.unbox();
        let binding_id = assign_pat.left.get_binding_identifier().unwrap();
        AssignmentTargetProperty::new_assignment_target_property_identifier(
          self.span,
          IdentifierReference::new(binding_id.span, binding_id.name, builder),
          Some(assign_pat.right),
          builder,
        )
      }
      // `{ a }`: a shorthand property
      BindingPattern::BindingIdentifier(id) if self.shorthand => {
        AssignmentTargetProperty::new_assignment_target_property_identifier(
          self.span,
          IdentifierReference::new(id.span, id.name, builder),
          None,
          builder,
        )
      }
      // `{ k: a }`, `{ k: [a] }` or `{ k: { a } }`, each with an optional default. The value has
      // the same grammar as an array element, so it uses the same conversion.
      value => AssignmentTargetProperty::new_assignment_target_property_property(
        self.span,
        self.key,
        value.into_assignment_target_maybe_default(builder),
        self.computed,
        builder,
      )
      .into_in(builder.allocator()),
    }
  }
}
