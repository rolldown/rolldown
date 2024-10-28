use oxc::allocator::{Allocator, Box, IntoIn};
use oxc::ast::ast;
use oxc::semantic::SymbolId;
use oxc::span::SPAN;
use smallvec::SmallVec;

use crate::allocator_helpers::take_in::TakeIn;
use crate::AstSnippet;

pub mod allocator_ext;
pub mod span_ext;

pub trait BindingIdentifierExt {
  fn expect_symbol_id(&self) -> SymbolId;
}

impl BindingIdentifierExt for ast::BindingIdentifier<'_> {
  #[inline]
  fn expect_symbol_id(&self) -> SymbolId {
    self.symbol_id.get().unwrap_or_else(|| panic!("fail get symbol id from {self:?}"))
  }
}

pub trait BindingPatternExt<'ast> {
  fn binding_identifiers(&self) -> smallvec::SmallVec<[&Box<ast::BindingIdentifier<'ast>>; 1]>;

  fn into_assignment_target(self, alloc: &'ast Allocator) -> ast::AssignmentTarget<'ast>;
}

impl<'ast> BindingPatternExt<'ast> for ast::BindingPattern<'ast> {
  fn binding_identifiers(&self) -> smallvec::SmallVec<[&Box<ast::BindingIdentifier<'ast>>; 1]> {
    let mut queue = vec![&self.kind];
    let mut ret = SmallVec::default();
    while let Some(binding_kind) = queue.pop() {
      match binding_kind {
        ast::BindingPatternKind::BindingIdentifier(id) => {
          ret.push(id);
        }
        ast::BindingPatternKind::ArrayPattern(arr_pat) => {
          queue.extend(arr_pat.elements.iter().flatten().map(|pat| &pat.kind).rev());
        }
        ast::BindingPatternKind::ObjectPattern(obj_pat) => {
          queue.extend(obj_pat.properties.iter().map(|prop| &prop.value.kind).rev());
        }
        //
        ast::BindingPatternKind::AssignmentPattern(assign_pat) => {
          queue.push(&assign_pat.left.kind);
        }
      };
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
                      name: assign_pat.left.get_identifier().unwrap(),
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
                      name: id.name.clone(),
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

pub trait StatementExt<'me, 'ast> {
  fn is_import_declaration(&self) -> bool;
  fn as_import_declaration(&'me self) -> Option<&'me ast::ImportDeclaration<'ast>>;
  fn as_export_default_declaration_mut(
    &'me mut self,
  ) -> Option<&'me mut ast::ExportDefaultDeclaration<'ast>>;
  fn as_export_all_declaration(&self) -> Option<&ast::ExportAllDeclaration<'ast>>;
  fn as_export_named_declaration(&self) -> Option<&ast::ExportNamedDeclaration<'ast>>;
  fn as_export_named_declaration_mut(&mut self) -> Option<&mut ast::ExportNamedDeclaration<'ast>>;

  fn is_function_declaration(&self) -> bool;
  fn as_function_declaration(&self) -> Option<&ast::Function<'ast>>;

  fn is_module_declaration_with_source(&self) -> bool;
}

impl<'me, 'ast> StatementExt<'me, 'ast> for ast::Statement<'ast> {
  fn is_import_declaration(&self) -> bool {
    matches!(self, ast::Statement::ImportDeclaration(_))
  }

  fn as_import_declaration(&self) -> Option<&ast::ImportDeclaration<'ast>> {
    if let ast::Statement::ImportDeclaration(import_decl) = self {
      return Some(&**import_decl);
    }
    None
  }

  fn as_export_default_declaration_mut(
    &mut self,
  ) -> Option<&mut ast::ExportDefaultDeclaration<'ast>> {
    if let ast::Statement::ExportDefaultDeclaration(export_default_decl) = self {
      return Some(&mut **export_default_decl);
    }
    None
  }

  fn as_export_all_declaration(&self) -> Option<&ast::ExportAllDeclaration<'ast>> {
    if let ast::Statement::ExportAllDeclaration(export_all_decl) = self {
      return Some(&**export_all_decl);
    }
    None
  }

  fn as_export_named_declaration(&self) -> Option<&ast::ExportNamedDeclaration<'ast>> {
    if let ast::Statement::ExportNamedDeclaration(export_named_decl) = self {
      return Some(&**export_named_decl);
    }
    None
  }

  fn as_export_named_declaration_mut(&mut self) -> Option<&mut ast::ExportNamedDeclaration<'ast>> {
    if let ast::Statement::ExportNamedDeclaration(export_named_decl) = self {
      return Some(&mut **export_named_decl);
    }
    None
  }

  fn as_function_declaration(&self) -> Option<&ast::Function<'ast>> {
    if let ast::Statement::FunctionDeclaration(func_decl) = self {
      Some(func_decl)
    } else {
      None
    }
  }

  fn is_function_declaration(&self) -> bool {
    self.as_function_declaration().is_some()
  }

  /// Check if the statement is `[import|export] ... from ...` or `export ... from ...`
  fn is_module_declaration_with_source(&self) -> bool {
    matches!(self.as_module_declaration(), Some(decl) if decl.source().is_some())
  }
}

pub trait ExpressionExt<'ast> {
  fn as_call_expression(&self) -> Option<&ast::CallExpression<'ast>>;
  fn as_call_expression_mut(&mut self) -> Option<&mut ast::CallExpression<'ast>>;

  fn as_identifier(&self) -> Option<&ast::IdentifierReference<'ast>>;
  fn as_identifier_mut(&mut self) -> Option<&mut ast::IdentifierReference<'ast>>;
  fn as_unary_expression(&self) -> Option<&ast::UnaryExpression<'ast>>;
  fn as_string_literal(&self) -> Option<&ast::StringLiteral<'ast>>;
  fn as_binary_expression(&self) -> Option<&ast::BinaryExpression<'ast>>;
}

impl<'ast> ExpressionExt<'ast> for ast::Expression<'ast> {
  fn as_call_expression(&self) -> Option<&ast::CallExpression<'ast>> {
    if let ast::Expression::CallExpression(call_expr) = self {
      Some(call_expr)
    } else {
      None
    }
  }

  fn as_call_expression_mut(&mut self) -> Option<&mut ast::CallExpression<'ast>> {
    if let ast::Expression::CallExpression(call_expr) = self {
      Some(call_expr)
    } else {
      None
    }
  }

  fn as_identifier(&self) -> Option<&ast::IdentifierReference<'ast>> {
    if let ast::Expression::Identifier(ident) = self {
      Some(ident)
    } else {
      None
    }
  }

  fn as_identifier_mut(&mut self) -> Option<&mut ast::IdentifierReference<'ast>> {
    if let ast::Expression::Identifier(ident) = self {
      Some(ident)
    } else {
      None
    }
  }

  fn as_unary_expression(&self) -> Option<&ast::UnaryExpression<'ast>> {
    let ast::Expression::UnaryExpression(expr) = self else {
      return None;
    };
    Some(expr)
  }

  fn as_string_literal(&self) -> Option<&ast::StringLiteral<'ast>> {
    let ast::Expression::StringLiteral(expr) = self else {
      return None;
    };
    Some(expr)
  }

  fn as_binary_expression(&self) -> Option<&ast::BinaryExpression<'ast>> {
    let ast::Expression::BinaryExpression(expr) = self else {
      return None;
    };
    Some(expr)
  }
}
