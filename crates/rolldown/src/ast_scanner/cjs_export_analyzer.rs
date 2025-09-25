use oxc::allocator::GetAddress;
use oxc::ast::{
  AstKind, MemberExpressionKind,
  ast::{self, AssignmentExpression, Expression, PropertyKey},
};
use oxc::span::CompactStr;
use rolldown_common::{AstScopes, EcmaModuleAstUsage};
use rolldown_ecmascript_utils::ExpressionExt;

use crate::ast_scanner::IdentifierReferenceKind;

use super::AstScanner;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonJsAstType {
  /// We don't need extra `module.exports` related type for now.
  /// If `CompactStr` eq = `*`, it means the property name is not a static string.
  ExportsPropWrite(CompactStr),
  /// Read global `exports` object, but not write to it. e.g.
  /// `console.log(exports)`
  ExportsRead,
  EsModuleFlag,
  Reexport,
}

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  pub fn commonjs_export_analyzer(&self, ty: &CjsGlobalAssignmentType) -> Option<CommonJsAstType> {
    let cursor = self.visit_path.len() - 1;
    let parent = self.visit_path.get(cursor)?;
    match parent {
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
              let parent_parent = parent_parent_kind.as_member_expression_kind().unwrap();
              Self::check_assignment_target_property(
                &parent_parent,
                self.visit_path.get(cursor - 2)?,
              )
            }
            AstKind::Argument(arg) => self.check_object_define_property(arg, cursor - 1),
            AstKind::AssignmentExpression(assignment_expr) => {
              self.check_assignment_is_cjs_reexport(assignment_expr)
            }
            _ => None,
          }
        }
        CjsGlobalAssignmentType::ExportsAssignment => {
          // one scenario:
          // 1. exports.__esModule = true;
          let member_expr = kind.as_member_expression_kind().unwrap();
          Self::check_assignment_target_property(&member_expr, self.visit_path.get(cursor - 1)?)
        }
      },
      AstKind::Argument(arg) => {
        // one scenario:
        // 1. Object.defineProperty(exports, "__esModule", { value: true });
        self.check_object_define_property(arg, cursor)
      }
      _ => None,
    }
  }

  pub fn update_ast_usage_for_commonjs_export(&mut self, v: Option<&CommonJsAstType>) {
    match v.as_ref() {
      Some(CommonJsAstType::EsModuleFlag) => {
        self.result.ast_usage.insert(EcmaModuleAstUsage::EsModuleFlag);
      }
      Some(CommonJsAstType::ExportsRead) => {
        self.result.ast_usage.remove(EcmaModuleAstUsage::AllStaticExportPropertyAccess);
      }
      Some(CommonJsAstType::ExportsPropWrite(prop)) if prop == "*" => {
        self.result.ast_usage.remove(EcmaModuleAstUsage::AllStaticExportPropertyAccess);
      }
      Some(CommonJsAstType::Reexport) => {
        self.result.ast_usage.insert(EcmaModuleAstUsage::IsCjsReexport);
      }
      _ => {}
    }
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
    member_expr: &MemberExpressionKind,
    parent: &AstKind<'ast>,
  ) -> Option<CommonJsAstType> {
    let static_property_name = member_expr.static_property_name();

    if !member_expr.is_assigned_to_in_parent(parent) {
      return Some(CommonJsAstType::ExportsRead);
    }

    let Some(static_property_name) = static_property_name else {
      return Some(CommonJsAstType::ExportsPropWrite(CompactStr::from("*")));
    };
    if static_property_name.as_str() != "__esModule" {
      return Some(CommonJsAstType::ExportsPropWrite(CompactStr::from(static_property_name)));
    }

    let assignment_expr = parent.as_assignment_expression()?;

    let Expression::BooleanLiteral(bool_lit) = &assignment_expr.right else {
      return Some(CommonJsAstType::ExportsPropWrite("__esModule".into()));
    };
    bool_lit.value.then_some(CommonJsAstType::EsModuleFlag)
  }

  /// check if the `module` is used as : module.exports = require('mod');
  fn check_assignment_is_cjs_reexport(
    &self,
    assignment_expr: &AssignmentExpression<'ast>,
  ) -> Option<CommonJsAstType> {
    let call_expr = assignment_expr.right.as_call_expression()?;
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
  let Some(string_lit) = second.as_expression().and_then(|item| item.as_string_literal()) else {
    return Some(CommonJsAstType::ExportsPropWrite("*".into()));
  };
  if string_lit.value != "__esModule" {
    return Some(CommonJsAstType::ExportsPropWrite(string_lit.value.as_str().into()));
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
  if ret {
    Some(CommonJsAstType::EsModuleFlag)
  } else {
    Some(CommonJsAstType::ExportsPropWrite("__esModule".into()))
  }
}

#[cfg(test)]
mod tests {

  use super::*;
  use oxc::{
    allocator::Allocator, ast::ast::Program, parser::Parser, semantic::SemanticBuilder,
    span::SourceType,
  };
  use rolldown_common::AstScopes;

  fn create_ast_scopes_and_program_from_source<'ast, 'a: 'ast>(
    source: &'ast str,
    allocator: &'a Allocator,
  ) -> (AstScopes, Program<'ast>) {
    let source_type = SourceType::default();
    let ret = Parser::new(allocator, source, source_type).parse();
    let program = ret.program;
    let semantic_ret = SemanticBuilder::new().build(&program);
    (AstScopes::new(semantic_ret.semantic.into_scoping()), program)
  }

  fn extract_call_expr<'a>(
    program: &'a oxc::ast::ast::Program<'a>,
  ) -> Option<&'a oxc::ast::ast::CallExpression<'a>> {
    let first = program.body.first()?;
    let oxc::ast::ast::Statement::ExpressionStatement(expr_stmt) = first else {
      return None;
    };
    expr_stmt.expression.as_call_expression()
  }

  #[test]
  fn test_is_object_define_property_es_module_valid() {
    let source = r#"Object.defineProperty(exports, "__esModule", { value: true });"#;
    let allocator = Allocator::default();
    let (ast_scopes, program) = create_ast_scopes_and_program_from_source(source, &allocator);

    if let Some(call_expr) = extract_call_expr(&program) {
      let result = is_object_define_property_es_module(&ast_scopes, call_expr);
      assert_eq!(result, Some(CommonJsAstType::EsModuleFlag));
    }
  }

  #[test]
  fn test_is_object_define_property_es_module_invalid() {
    let source = r#"Object.defineProperty(exports, "notEsModule", { value: true });"#;
    let allocator = Allocator::default();
    let (ast_scopes, program) = create_ast_scopes_and_program_from_source(source, &allocator);

    if let Some(call_expr) = extract_call_expr(&program) {
      let result = is_object_define_property_es_module(&ast_scopes, call_expr);
      assert_eq!(result, Some(CommonJsAstType::ExportsPropWrite("notEsModule".into())));
    }
  }

  #[test]
  fn test_is_object_define_property_with_false_value() {
    let source = r#"Object.defineProperty(exports, "__esModule", { value: false });"#;
    let allocator = Allocator::default();
    let (ast_scopes, program) = create_ast_scopes_and_program_from_source(source, &allocator);

    if let Some(call_expr) = extract_call_expr(&program) {
      let result = is_object_define_property_es_module(&ast_scopes, call_expr);
      assert_eq!(result, Some(CommonJsAstType::ExportsPropWrite("__esModule".into())));
    }
  }
}
