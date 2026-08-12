use oxc::ast::ast::{
  Argument, BindingPattern, CallExpression, Expression, ImportDeclaration,
  ImportDeclarationSpecifier, ObjectPropertyKind, PropertyKey, Statement, VariableDeclaration,
};
use oxc::span::GetSpan;
use rustc_hash::FxHashMap;

use crate::codegen;

pub struct HelperTransformer;

impl HelperTransformer {
  pub fn collect_export_mappings_public(
    stmt: &Statement,
    mappings: &mut FxHashMap<String, String>,
  ) {
    Self::collect_export_mappings(stmt, mappings);
  }

  pub fn is_helper_import_public(import: &ImportDeclaration) -> bool {
    Self::is_helper_import(import)
  }

  pub fn transform_export_all_public(
    var_decl: &VariableDeclaration,
    source: &str,
  ) -> Option<String> {
    let result = Self::transform_export_all(var_decl, source)?;
    let original = codegen::extract_span_text(source, var_decl.span());
    if result == original { None } else { Some(result) }
  }

  pub fn transform_member_access_public(
    var_decl: &VariableDeclaration,
    export_mappings: &FxHashMap<String, String>,
  ) -> Option<String> {
    if !Self::is_member_access(var_decl) {
      return None;
    }
    Self::transform_member_access(var_decl, export_mappings)
  }

  fn collect_export_mappings(stmt: &Statement, mappings: &mut FxHashMap<String, String>) {
    match stmt {
      Statement::ImportDeclaration(import) => {
        if let Some(specifiers) = &import.specifiers {
          if specifiers.len() == 1 {
            if let Some(ImportDeclarationSpecifier::ImportSpecifier(spec)) = specifiers.first() {
              let local_name = spec.local.name.to_string();
              if local_name.ends_with("_exports") {
                mappings.insert(local_name.clone(), local_name);
              }
            }
          }
        }
      }
      Statement::ExpressionStatement(expr_stmt) => {
        if let Expression::CallExpression(call) = &expr_stmt.expression {
          if Self::is_re_export_call(call) {
            if let (Some(first_arg), Some(second_arg)) =
              (call.arguments.first(), call.arguments.get(1))
            {
              if let (Argument::Identifier(first_id), Argument::Identifier(second_id)) =
                (first_arg, second_arg)
              {
                mappings.insert(first_id.name.to_string(), second_id.name.to_string());
              }
            }
          }
        }
      }
      _ => {}
    }
  }

  fn transform_export_all(var_decl: &VariableDeclaration, source: &str) -> Option<String> {
    if var_decl.declarations.len() != 1 {
      return Some(codegen::extract_span_text(source, var_decl.span()));
    }

    let declarator = &var_decl.declarations[0];

    let init = declarator.init.as_ref()?;
    if let Expression::CallExpression(call) = init {
      if !Self::is_export_all_call(call) {
        return Some(codegen::extract_span_text(source, var_decl.span()));
      }

      let ns_name = match &declarator.id {
        BindingPattern::BindingIdentifier(id) => id.name.to_string(),
        _ => return Some(codegen::extract_span_text(source, var_decl.span())),
      };

      if let Some(Argument::ObjectExpression(obj)) = call.arguments.first() {
        let mut exports = Vec::new();

        for prop in &obj.properties {
          if let ObjectPropertyKind::ObjectProperty(prop) = prop {
            let exported = match &prop.key {
              PropertyKey::Identifier(id) => id.name.to_string(),
              PropertyKey::StaticIdentifier(id) => id.name.to_string(),
              PropertyKey::StringLiteral(lit) => serde_json::to_string(lit.value.as_str())
                .unwrap_or_else(|_| format!("\"{}\"", lit.value.escape_default())),
              _ => continue,
            };
            let exported_is_string = matches!(&prop.key, PropertyKey::StringLiteral(_));

            if let Expression::ArrowFunctionExpression(arrow) = &prop.value
              && let Some(Expression::Identifier(id)) = arrow.get_expression()
            {
              let local = id.name.to_string();
              if !exported_is_string && local == exported {
                exports.push(local);
              } else {
                exports.push(format!("{local} as {exported}"));
              }
            }
          }
        }

        if exports.is_empty() {
          return Some(format!("declare namespace {ns_name} {{}}"));
        }

        let exports_str = exports.join(", ");
        return Some(format!("declare namespace {ns_name} {{\n  export {{ {exports_str} }};\n}}"));
      }
    }

    Some(codegen::extract_span_text(source, var_decl.span()))
  }

  fn transform_member_access(
    var_decl: &VariableDeclaration,
    export_mappings: &FxHashMap<String, String>,
  ) -> Option<String> {
    if var_decl.declarations.len() != 1 {
      return None;
    }

    let declarator = &var_decl.declarations[0];
    let init = declarator.init.as_ref()?;

    if let Some(member) = init.as_member_expression() {
      if let Expression::Identifier(obj_id) = member.object() {
        let obj_name = obj_id.name.to_string();

        if let Some(mapped_name) = export_mappings.get(&obj_name) {
          let binding_name = match &declarator.id {
            BindingPattern::BindingIdentifier(id) => id.name.to_string(),
            _ => return None,
          };

          if let Some(prop_name) = member.static_property_name() {
            return Some(format!("type {binding_name} = {mapped_name}.{prop_name};"));
          }
        }
      }
    }

    None
  }

  fn is_export_all_call(call: &CallExpression) -> bool {
    if let Expression::Identifier(id) = &call.callee { id.name == "__exportAll" } else { false }
  }

  fn is_re_export_call(call: &CallExpression) -> bool {
    if let Expression::Identifier(id) = &call.callee { id.name == "__reExport" } else { false }
  }

  fn is_helper_import(import: &ImportDeclaration) -> bool {
    if let Some(specifiers) = &import.specifiers {
      if specifiers.len() != 1 {
        return false;
      }

      if let Some(ImportDeclarationSpecifier::ImportSpecifier(spec)) = specifiers.first() {
        let name = spec.local.name.as_str();
        return name == "__exportAll" || name == "__reExport";
      }
    }
    false
  }

  fn is_member_access(var_decl: &VariableDeclaration) -> bool {
    if var_decl.declarations.len() != 1 {
      return false;
    }

    if let Some(init) = &var_decl.declarations[0].init {
      init.as_member_expression().is_some()
    } else {
      false
    }
  }
}
