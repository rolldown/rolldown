use oxc::{
  allocator::{Allocator, Box, IntoIn},
  ast::ast,
  span::SPAN,
};
use smallvec::SmallVec;

use crate::{AstSnippet, TakeIn};

pub trait BindingPatternExt<'ast> {
  fn binding_identifiers(&self) -> smallvec::SmallVec<[&Box<ast::BindingIdentifier<'ast>>; 1]>;

  fn into_assignment_target(self, alloc: &'ast Allocator) -> ast::AssignmentTarget<'ast>;
}

impl<'ast> BindingPatternExt<'ast> for ast::BindingPattern<'ast> {
  fn binding_identifiers(&self) -> smallvec::SmallVec<[&Box<ast::BindingIdentifier<'ast>>; 1]> {
    let mut stack = vec![&self.kind];
    let mut ret = SmallVec::default();
    while let Some(binding_kind) = stack.pop() {
      match binding_kind {
        ast::BindingPatternKind::BindingIdentifier(id) => {
          ret.push(id);
        }
        ast::BindingPatternKind::ArrayPattern(arr_pat) => {
          stack.extend(arr_pat.elements.iter().flatten().map(|pat| &pat.kind).rev());
        }
        ast::BindingPatternKind::ObjectPattern(obj_pat) => {
          if let Some(obj_pat) = &obj_pat.rest {
            stack.push(&obj_pat.argument.kind);
          }
          stack.extend(obj_pat.properties.iter().map(|prop| &prop.value.kind).rev());
        }
        //
        ast::BindingPatternKind::AssignmentPattern(assign_pat) => {
          stack.push(&assign_pat.left.kind);
        }
      }
    }
    ret
  }

  #[allow(clippy::too_many_lines)]
  fn into_assignment_target(mut self, alloc: &'ast Allocator) -> ast::AssignmentTarget<'ast> {
    let left = match &mut self.kind {
      // Turn `var a = 1` into `a = 1`
      ast::BindingPatternKind::BindingIdentifier(id) => {
        AstSnippet::new(alloc).simple_id_assignment_target(&id.name, id.span)
      }
      // Turn `var { a, b = 2 } = ...` to `{a, b = 2} = ...`
      ast::BindingPatternKind::ObjectPattern(obj_pat) => {
        let mut obj_target = ast::ObjectAssignmentTarget {
          rest: obj_pat.rest.take().map(|rest| ast::AssignmentTargetRest {
            span: SPAN,
            target: rest.unbox().argument.into_assignment_target(alloc),
          }),
          ..TakeIn::dummy(alloc)
        };

        obj_pat.properties.take_in(alloc).into_iter().for_each(|binding_prop| {
          obj_target.properties.push(match binding_prop.value.kind {
            ast::BindingPatternKind::AssignmentPattern(assign_pat) => {
              let assign_pat = assign_pat.unbox();

              if binding_prop.shorthand {
                ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
                  ast::AssignmentTargetPropertyIdentifier {
                    binding: ast::IdentifierReference {
                      name: assign_pat.left.get_identifier_name().unwrap(),
                      ..TakeIn::dummy(alloc)
                    },
                    init: Some(assign_pat.right),
                    ..TakeIn::dummy(alloc)
                  }
                  .into_in(alloc),
                )
              } else {
                ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(
                  ast::AssignmentTargetPropertyProperty {
                    name: binding_prop.key,
                    binding: ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(
                      ast::AssignmentTargetWithDefault {
                        binding: assign_pat.left.into_assignment_target(alloc),
                        init: assign_pat.right,
                        ..TakeIn::dummy(alloc)
                      }
                      .into_in(alloc),
                    ),
                    ..TakeIn::dummy(alloc)
                  }
                  .into_in(alloc),
                )
                .into_in(alloc)
              }
            }
            ast::BindingPatternKind::BindingIdentifier(ref id) => {
              if binding_prop.shorthand {
                ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
                  ast::AssignmentTargetPropertyIdentifier {
                    binding: ast::IdentifierReference {
                      name: id.name,
                      ..TakeIn::dummy(alloc)
                    },
                    init: None,
                    ..TakeIn::dummy(alloc)
                  }
                  .into_in(alloc),
                )
              } else {
                ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(
                  ast::AssignmentTargetPropertyProperty {
                    name: binding_prop.key,
                    binding: ast::AssignmentTargetMaybeDefault::from(
                      binding_prop.value.into_assignment_target(alloc),
                    ),
                    ..TakeIn::dummy(alloc)
                  }
                  .into_in(alloc),
                )
                .into_in(alloc)
              }
            }
            _ => {
              unreachable!(
                "The kind of `BindingProperty`'s value should not be `ObjectPattern` and `ArrayPattern`"
              )
            }
          });
        });

        ast::AssignmentTarget::ObjectAssignmentTarget(obj_target.into_in(alloc))
      }
      // Turn `var [a, ,c = 1] = ...` to `[a, ,c = 1] = ...`
      ast::BindingPatternKind::ArrayPattern(arr_pat) => {
        let mut arr_target = ast::ArrayAssignmentTarget {
          rest: arr_pat.rest.take().map(|rest| ast::AssignmentTargetRest {
            span: SPAN,
            target: rest.unbox().argument.into_assignment_target(alloc),
          }),
          ..TakeIn::dummy(alloc)
        };
        arr_pat.elements.take_in(alloc).into_iter().for_each(|binding_pat| {
          arr_target.elements.push(binding_pat.map(|binding_pat| match binding_pat.kind {
            ast::BindingPatternKind::AssignmentPattern(assign_pat) => {
              let assign_pat = assign_pat.unbox();
              ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(
                ast::AssignmentTargetWithDefault {
                  binding: assign_pat.left.into_assignment_target(alloc),
                  init: assign_pat.right,
                  ..TakeIn::dummy(alloc)
                }
                .into_in(alloc),
              )
            }
            _ => ast::AssignmentTargetMaybeDefault::from(binding_pat.into_assignment_target(alloc)),
          }));
        });
        ast::AssignmentTarget::ArrayAssignmentTarget(arr_target.into_in(alloc))
      }
      ast::BindingPatternKind::AssignmentPattern(_) => {
        unreachable!("`BindingPatternKind::AssignmentPattern` should be pre-handled in above")
      }
    };
    left
  }
}
