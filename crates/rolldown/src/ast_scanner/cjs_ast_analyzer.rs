use oxc::allocator::GetAddress;
use oxc::ast::MemberExpressionKind;
use oxc::ast::{
  AstKind,
  ast::{self, Expression, PropertyKey},
};
use rolldown_common::{AstScopes, EcmaModuleAstUsage};
use rolldown_ecmascript_utils::ExpressionExt;

use crate::ast_scanner::IdentifierReferenceKind;

use super::AstScanner;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonJsAstType {
  // We don't need extra `module.exports` related type for now.
  ExportsPropWrite,
  ExportsRead,
  EsModuleFlag,
  Reexport,
}

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  #[allow(clippy::too_many_lines)]
  pub fn cjs_ast_analyzer(&mut self, ty: &CjsGlobalAssignmentType) -> Option<CommonJsAstType> {
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
      kind if kind.is_member_expression_kind() => match ty {
        // two scenarios:
        // 1. module.exports.__esModule = true;
        // 2. Object.defineProperty(module.exports, "__esModule", { value: true });
        CjsGlobalAssignmentType::ModuleExportsAssignment => {
          let member_expr = kind.as_member_expression_kind().unwrap();
          let property_name = member_expr.static_property_name()?;
          if property_name != "exports" {
            return None;
          }
          let parent_parent_kind = self.visit_path.get(cursor - 1)?;
          match parent_parent_kind {
            parent_parent_kind if parent_parent_kind.is_member_expression_kind() => {
              let parent_parent = kind.as_member_expression_kind().unwrap();
              self.check_assignment_target_property(&parent_parent, cursor - 1)
            }
            AstKind::Argument(arg) => self.check_object_define_property(arg, cursor - 1),
            AstKind::SimpleAssignmentTarget(target) => {
              let v = self.check_assignment_is_cjs_reexport(target, cursor - 1);
              if matches!(v, Some(CommonJsAstType::Reexport)) {
                self.ast_usage.insert(EcmaModuleAstUsage::IsCjsReexport);
              }
              v
            }
            _ => None,
          }
        }
        CjsGlobalAssignmentType::ExportsAssignment => {
          // one scenario:
          // 1. exports.__esModule = true;
          let member_expr = kind.as_member_expression_kind().unwrap();
          self.check_assignment_target_property(&member_expr, cursor)
        }
      },
      AstKind::Argument(arg) => {
        // one scenario:
        // 1. Object.defineProperty(exports, "__esModule", { value: true });
        self.check_object_define_property(arg, cursor)
      }
      _ => None,
    };
    if matches!(v, Some(CommonJsAstType::EsModuleFlag)) {
      self.ast_usage.insert(EcmaModuleAstUsage::EsModuleFlag);
    }
    v
  }

  /// Check if the argument is a valid `Object.defineProperty` call expression for `__esModule` flag.
  fn check_object_define_property(
    &self,
    arg: &ast::Argument<'_>,
    base_cursor: usize,
  ) -> Option<CommonJsAstType> {
    let call_expr = self.visit_path.get(base_cursor - 1)?.as_call_expression()?;

    let first = call_expr.arguments.first()?;
    let is_same_member_expr = arg.address() == first.address();
    if !is_same_member_expr {
      return None;
    }
    is_object_define_property_es_module(&self.result.symbol_ref_db.ast_scopes, call_expr)
  }

  /// Check if the member expression is a valid assignment target for `__esModule` flag.
  fn check_assignment_target_property(
    &mut self,
    member_expr: &MemberExpressionKind,
    base_cursor: usize,
  ) -> Option<CommonJsAstType> {
    let static_property_name = member_expr.static_property_name();
    if static_property_name.is_none() {
      self.ast_usage.remove(EcmaModuleAstUsage::AllStaticExportPropertyAccess);
    }
    let is_es_module_flag_prop =
      static_property_name.is_some_and(|atom| atom.as_str() == "__esModule");

    match self.visit_path.get(base_cursor - 1)?.as_simple_assignment_target() {
      Some(_) => {
        if !is_es_module_flag_prop {
          return Some(CommonJsAstType::ExportsPropWrite);
        }
      }
      None => {
        return None;
      }
    }
    self.visit_path.get(base_cursor - 2)?.as_assignment_target()?;

    let assignment_expr = self.visit_path.get(base_cursor - 3)?.as_assignment_expression()?;

    let ast::Expression::BooleanLiteral(bool_lit) = &assignment_expr.right else {
      return None;
    };
    bool_lit.value.then_some(CommonJsAstType::EsModuleFlag)
  }

  /// check if the `module` is used as : module.exports = require('mod');
  fn check_assignment_is_cjs_reexport(
    &self,
    _target: &ast::SimpleAssignmentTarget<'_>,
    base_cursor: usize,
  ) -> Option<CommonJsAstType> {
    self.visit_path.get(base_cursor - 1)?.as_assignment_target()?;

    let assignment_expr = self.visit_path.get(base_cursor - 2)?.as_assignment_expression()?;
    let ast::Expression::CallExpression(call_expr) = &assignment_expr.right else {
      return None;
    };
    let callee = call_expr.callee.as_identifier()?;

    if !(callee.name == "require"
      && matches!(self.resolve_identifier_reference(callee), IdentifierReferenceKind::Global,)
      && call_expr.arguments.len() == 1)
    {
      return None;
    }
    call_expr
      .arguments
      .first()?
      .as_expression()?
      .as_string_literal()
      .is_some()
      .then_some(CommonJsAstType::Reexport)
  }
}

pub enum CjsGlobalAssignmentType {
  ModuleExportsAssignment,
  ExportsAssignment,
}

/// check if the `CallExpression` is Object.defineProperty(exports, "__esModule", { value: true });
pub fn is_object_define_property_es_module(
  scope: &AstScopes,
  call_expr: &ast::CallExpression<'_>,
) -> Option<CommonJsAstType> {
  let callee = call_expr.callee.as_member_expression()?;
  let callee_object = callee.object().as_identifier()?;
  // Check if it is global variable `Object`.
  if !scope.is_unresolved(callee_object.reference_id()) {
    return None;
  }
  let key_eq_object = callee_object.name == "Object";
  let property_eq_define_property = callee.static_property_name()? == "defineProperty";
  if !(key_eq_object && property_eq_define_property) {
    return Some(CommonJsAstType::ExportsRead);
  }
  let first = call_expr.arguments.first()?.as_expression()?.as_identifier()?;

  if !scope.is_unresolved(first.reference_id()) || first.name != "exports" {
    return None;
  }

  let second = call_expr.arguments.get(1)?;
  let is_es_module = second
    .as_expression()
    .and_then(|item| item.as_string_literal())
    .is_some_and(|item| item.value == "__esModule");
  if !is_es_module {
    return Some(CommonJsAstType::ExportsRead);
  }
  let third = call_expr.arguments.get(2)?;
  let ret = third
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
  if ret { Some(CommonJsAstType::EsModuleFlag) } else { Some(CommonJsAstType::ExportsRead) }
}
