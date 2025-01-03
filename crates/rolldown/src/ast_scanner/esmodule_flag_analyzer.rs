use oxc::ast::{
  ast::{self, Expression, IdentifierReference, PropertyKey},
  AstKind,
};
use rolldown_ecmascript_utils::ExpressionExt;

use super::AstScanner;

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  pub fn check_es_module_flag(
    &self,
    ident: &IdentifierReference<'ast>,
    ty: EsModuleFlagCheckType,
  ) -> Option<bool> {
    let cursor = self.visit_path.len() - 1;
    let parent = self.visit_path.get(cursor)?;
    match parent {
      AstKind::MemberExpression(member_expr) => match ty {
        // two scenarios:
        // 1. module.exports.__esModule = true;
        // 2. Object.defineProperty(module.exports, "__esModule", { value: true });
        EsModuleFlagCheckType::ModuleExportsAssignment => {
          let property_name = member_expr.static_property_name()?;
          if property_name != "exports" {
            return Some(false);
          }
          let parent_parent_kind = self.visit_path.get(cursor - 1)?;
          match parent_parent_kind {
            AstKind::MemberExpression(parent_parent) => {
              let property_name = parent_parent.static_property_name()?;
              if property_name != "__esModule" {
                return Some(false);
              }
              let parent_parent_parent_kind = self.visit_path.get(cursor - 2)?;
              let parent_parent_parent = parent_parent_parent_kind.as_assignment_expression()?;
              let ast::Expression::BooleanLiteral(bool_lit) = &parent_parent_parent.right else {
                return Some(false);
              };
              Some(bool_lit.value)
            }
            AstKind::CallExpression(parent_parent) => {
              let callee = parent_parent.callee.as_member_expression()?;
              let key_eq_object =
                callee.object().as_identifier().is_some_and(|item| item.name == "Object");
              let property_eq_define_property = callee.static_property_name()? == "defineProperty";
              if !(key_eq_object && property_eq_define_property) {
                return Some(false);
              }
              let first = parent_parent.arguments.get(0)?;
              let is_same_ident_ref = first
                .as_expression()
                .and_then(|item| item.as_identifier())
                .is_some_and(|item| item.reference_id() == ident.reference_id());
              if !is_same_ident_ref {
                return Some(false);
              }
              let second = parent_parent.arguments.get(1)?;
              let is_es_module = second
                .as_expression()
                .and_then(|item| item.as_string_literal())
                .is_some_and(|item| item.value == "__esModule");
              if !is_es_module {
                return Some(false);
              }
              let third = parent_parent.arguments.get(2)?;
              let flag = third
                .as_expression()
                .and_then(|item| match item {
                  Expression::ObjectExpression(expr) => Some(expr),
                  _ => None,
                })
                .is_some_and(|obj_expr| match obj_expr.properties.as_slice() {
                  [ast::ObjectPropertyKind::ObjectProperty(kind)] => match (&kind.key, &kind.value)
                  {
                    (PropertyKey::StaticIdentifier(id), Expression::BooleanLiteral(bool_lit)) => {
                      id.name == "value" && bool_lit.value
                    }
                    _ => false,
                  },
                  _ => false,
                });
              if !flag {
                return Some(false);
              }
              Some(true)
            }
            _ => None,
          }
        }
        // one scenario:
        // 1. exports.__esModule = true;
        EsModuleFlagCheckType::ExportsAssignment => {
          let property_name = member_expr.static_property_name()?;
          if property_name != "__esModule" {
            return Some(false);
          }
          let parent_parent_kind = self.visit_path.get(cursor - 1)?;
          let parent_parent = parent_parent_kind.as_assignment_expression()?;

          let ast::Expression::BooleanLiteral(bool_lit) = &parent_parent.right else {
            return Some(false);
          };
          Some(bool_lit.value)
        }
      },
      AstKind::CallExpression(call_expr) => {
        // one scenario:
        // 1. Object.defineProperty(exports, "__esModule", { value: true });
        let first = call_expr.arguments.get(0)?;
        let is_same_ident_ref = first
          .as_expression()
          .and_then(|item| item.as_identifier())
          .is_some_and(|item| item.reference_id() == ident.reference_id());
        if !is_same_ident_ref {
          return Some(false);
        }
        let second = call_expr.arguments.get(1)?;
        let is_es_module = second
          .as_expression()
          .and_then(|item| item.as_string_literal())
          .is_some_and(|item| item.value == "__esModule");
        if !is_es_module {
          return Some(false);
        }

        let third = call_expr.arguments.get(2)?;
        let flag = third
          .as_expression()
          .and_then(|item| match item {
            Expression::ObjectExpression(expr) => Some(expr),
            _ => None,
          })
          .is_some_and(|obj_expr| match obj_expr.properties.as_slice() {
            [ast::ObjectPropertyKind::ObjectProperty(kind)] => match (&kind.key, &kind.value) {
              (PropertyKey::StaticIdentifier(id), Expression::BooleanLiteral(bool_lit)) => {
                id.name == "value" && bool_lit.value
              }
              _ => false,
            },
            _ => false,
          });
        if !flag {
          return Some(false);
        }
        let callee = call_expr.callee.as_member_expression()?;
        let key_eq_object =
          callee.object().as_identifier().is_some_and(|item| item.name == "Object");
        let property_eq_define_property = callee.static_property_name()? == "defineProperty";
        Some(key_eq_object && property_eq_define_property)
      }
      _ => None,
    }
  }
}

pub enum EsModuleFlagCheckType {
  ModuleExportsAssignment,
  ExportsAssignment,
}
