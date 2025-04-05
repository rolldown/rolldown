use oxc::allocator::GetAddress;
use oxc::ast::{
  AstKind,
  ast::{self, Expression, PropertyKey},
};
use rolldown_common::EcmaModuleAstUsage;
use rolldown_ecmascript_utils::ExpressionExt;

use crate::ast_scanner::IdentifierReferenceKind;

use super::AstScanner;

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  #[allow(clippy::too_many_lines)]
  pub fn cjs_ast_analyzer(&mut self, ty: &CjsGlobalAssignmentType) -> Option<()> {
    match ty {
      CjsGlobalAssignmentType::ModuleExportsAssignment => {
        self.ast_usage.insert(EcmaModuleAstUsage::ModuleRef);
      }
      CjsGlobalAssignmentType::ExportsAssignment => {
        self.ast_usage.insert(EcmaModuleAstUsage::ExportsRef);
      }
    }
    let cursor = self.visit_path.len() - 1;
    let parent = self.visit_path.get(cursor)?;
    let v = match parent {
      AstKind::MemberExpression(member_expr) => match ty {
        // two scenarios:
        // 1. module.exports.__esModule = true;
        // 2. Object.defineProperty(module.exports, "__esModule", { value: true });
        CjsGlobalAssignmentType::ModuleExportsAssignment => {
          let property_name = member_expr.static_property_name()?;
          if property_name != "exports" {
            return None;
          }
          let parent_parent_kind = self.visit_path.get(cursor - 1)?;
          match parent_parent_kind {
            AstKind::MemberExpression(parent_parent) => {
              self.check_assignment_target_property(parent_parent, cursor - 1)
            }
            AstKind::Argument(arg) => self.check_object_define_property(arg, cursor - 1),
            AstKind::SimpleAssignmentTarget(target) => {
              if self.check_assignment_is_cjs_reexport(target, cursor - 1).unwrap_or_default() {
                self.ast_usage.insert(EcmaModuleAstUsage::IsCjsReexport);
              }
              None
            }
            _ => None,
          }
        }
        CjsGlobalAssignmentType::ExportsAssignment => {
          // one scenario:
          // 1. exports.__esModule = true;
          self.check_assignment_target_property(member_expr, cursor)
        }
      },
      AstKind::Argument(arg) => {
        // one scenario:
        // 1. Object.defineProperty(exports, "__esModule", { value: true });
        self.check_object_define_property(arg, cursor)
      }
      _ => None,
    };
    if v.unwrap_or_default() {
      self.ast_usage.insert(EcmaModuleAstUsage::EsModuleFlag);
    }
    None
  }

  /// Check if the argument is a valid `Object.defineProperty` call expression for `__esModule` flag.
  fn check_object_define_property(
    &self,
    arg: &ast::Argument<'_>,
    base_cursor: usize,
  ) -> Option<bool> {
    let call_expr = self.visit_path.get(base_cursor - 1)?.as_call_expression()?;
    let callee = call_expr.callee.as_member_expression()?;
    let key_eq_object = callee.object().as_identifier().is_some_and(|item| item.name == "Object");
    let property_eq_define_property = callee.static_property_name()? == "defineProperty";
    if !(key_eq_object && property_eq_define_property) {
      return Some(false);
    }
    let first = call_expr.arguments.first()?;
    let is_same_member_expr = arg.address() == first.address();
    if !is_same_member_expr {
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
    Some(flag)
  }

  /// Check if the member expression is a valid assignment target for `__esModule` flag.
  fn check_assignment_target_property(
    &mut self,
    member_expr: &ast::MemberExpression<'_>,
    base_cursor: usize,
  ) -> Option<bool> {
    let static_property_name = member_expr.static_property_name();
    if static_property_name.is_none() {
      self.ast_usage.remove(EcmaModuleAstUsage::AllStaticExportPropertyAccess);
    }
    if static_property_name != Some("__esModule") {
      return Some(false);
    }

    self.visit_path.get(base_cursor - 1)?.as_simple_assignment_target()?;
    self.visit_path.get(base_cursor - 2)?.as_assignment_target()?;

    let assignment_expr = self.visit_path.get(base_cursor - 3)?.as_assignment_expression()?;

    let ast::Expression::BooleanLiteral(bool_lit) = &assignment_expr.right else {
      return Some(false);
    };
    Some(bool_lit.value)
  }

  /// check if the `module` is used as : module.exports = require('mod');
  fn check_assignment_is_cjs_reexport(
    &self,
    _target: &ast::SimpleAssignmentTarget<'_>,
    base_cursor: usize,
  ) -> Option<bool> {
    self.visit_path.get(base_cursor - 1)?.as_assignment_target()?;

    let assignment_expr = self.visit_path.get(base_cursor - 2)?.as_assignment_expression()?;
    let ast::Expression::CallExpression(call_expr) = &assignment_expr.right else {
      return Some(false);
    };
    let Some(callee) = call_expr.callee.as_identifier() else {
      return Some(false);
    };
    if !(callee.name == "require"
      && matches!(self.resolve_identifier_reference(callee), IdentifierReferenceKind::Global,)
      && call_expr.arguments.len() == 1)
    {
      return Some(false);
    }
    Some(call_expr.arguments.first()?.as_expression()?.as_string_literal().is_some())
  }
}

pub enum CjsGlobalAssignmentType {
  ModuleExportsAssignment,
  ExportsAssignment,
}
