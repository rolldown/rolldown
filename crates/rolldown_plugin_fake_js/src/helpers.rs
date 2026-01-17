use oxc::ast::ast::{
  CallExpression, ExportNamedDeclaration, Expression, ImportDeclaration, Statement,
  VariableDeclaration,
};
use oxc::span::GetSpan;
use std::collections::HashMap;

pub struct HelperTransformer;

impl HelperTransformer {
  pub fn transform_statements(stmts: &[Statement], source: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut export_mappings = HashMap::new();

    for stmt in stmts {
      Self::collect_export_mappings(stmt, &mut export_mappings);
    }

    for stmt in stmts {
      if let Some(transformed) = Self::transform_statement(stmt, source, &export_mappings) {
        result.push(transformed);
      }
    }

    result
  }

  fn collect_export_mappings(stmt: &Statement, mappings: &mut HashMap<String, String>) {
    match stmt {
      Statement::ImportDeclaration(import) => {
        if let Some(specifiers) = &import.specifiers {
          if specifiers.len() == 1 {
            if let Some(oxc::ast::ast::ImportDeclarationSpecifier::ImportSpecifier(spec)) =
              specifiers.first()
            {
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
              if let (
                oxc::ast::ast::Argument::Identifier(first_id),
                oxc::ast::ast::Argument::Identifier(second_id),
              ) = (first_arg, second_arg)
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

  fn transform_statement(
    stmt: &Statement,
    source: &str,
    export_mappings: &HashMap<String, String>,
  ) -> Option<String> {
    match stmt {
      Statement::VariableDeclaration(var_decl) if Self::is_member_access(var_decl) => {
        Self::transform_member_access(var_decl, export_mappings)
      }
      Statement::VariableDeclaration(var_decl) => Self::transform_export_all(var_decl, source),
      Statement::ExportNamedDeclaration(export) => {
        Self::transform_export_with_mapping(export, source, export_mappings)
      }
      Statement::ImportDeclaration(import) if Self::is_helper_import(import) => None,
      Statement::ExpressionStatement(expr_stmt) => {
        if let Expression::CallExpression(call) = &expr_stmt.expression {
          if Self::is_re_export_call(call) {
            return None;
          }
        }
        Some(Self::extract_text(source, stmt.span()))
      }
      _ => Some(Self::extract_text(source, stmt.span())),
    }
  }

  fn transform_export_all(var_decl: &VariableDeclaration, source: &str) -> Option<String> {
    if var_decl.declarations.len() != 1 {
      return Some(Self::extract_text(source, var_decl.span()));
    }

    let declarator = &var_decl.declarations[0];

    let init = declarator.init.as_ref()?;
    if let Expression::CallExpression(call) = init {
      if !Self::is_export_all_call(call) {
        return Some(Self::extract_text(source, var_decl.span()));
      }

      let ns_name = match &declarator.id {
        oxc::ast::ast::BindingPattern::BindingIdentifier(id) => id.name.to_string(),
        _ => return Some(Self::extract_text(source, var_decl.span())),
      };

      if let Some(oxc::ast::ast::Argument::ObjectExpression(obj)) = call.arguments.first() {
        let mut exports = Vec::new();

        for prop in &obj.properties {
          if let oxc::ast::ast::ObjectPropertyKind::ObjectProperty(prop) = prop {
            let exported = match &prop.key {
              oxc::ast::ast::PropertyKey::Identifier(id) => id.name.to_string(),
              oxc::ast::ast::PropertyKey::StaticIdentifier(id) => id.name.to_string(),
              _ => continue,
            };

            if let Expression::ArrowFunctionExpression(arrow) = &prop.value {
              if arrow.expression && arrow.body.statements.len() == 1 {
                if let Some(oxc::ast::ast::Statement::ExpressionStatement(expr_stmt)) =
                  arrow.body.statements.first()
                {
                  if let Expression::Identifier(id) = &expr_stmt.expression {
                    let local = id.name.to_string();
                    exports.push(format!("{local} as {exported}"));
                  }
                }
              }
            }
          }
        }

        if exports.is_empty() {
          return Some(format!("declare namespace {ns_name} {{}}"));
        }

        return Some(format!(
          "declare namespace {ns_name} {{ export {{ {} }} }}",
          exports.join(", ")
        ));
      }
    }

    Some(Self::extract_text(source, var_decl.span()))
  }

  fn transform_member_access(
    var_decl: &VariableDeclaration,
    export_mappings: &HashMap<String, String>,
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
            oxc::ast::ast::BindingPattern::BindingIdentifier(id) => id.name.to_string(),
            _ => return None,
          };

          if let Some(prop_name) = member.static_property_name() {
            return Some(format!("type {binding_name} = {mapped_name}.{prop_name}"));
          }
        }
      }
    }

    None
  }

  fn transform_export_with_mapping(
    export: &ExportNamedDeclaration,
    source: &str,
    export_mappings: &HashMap<String, String>,
  ) -> Option<String> {
    if export.declaration.is_some() {
      return None;
    }

    if export.specifiers.len() != 1 {
      return Some(Self::extract_text(source, export.span()));
    }

    if let Some(spec) = export.specifiers.first() {
      let local_name = spec.local.name().to_string();

      if let Some(mapped_name) = export_mappings.get(&local_name) {
        let exported_name = match &spec.exported {
          oxc::ast::ast::ModuleExportName::IdentifierName(id) => id.name.to_string(),
          oxc::ast::ast::ModuleExportName::IdentifierReference(id) => id.name.to_string(),
          oxc::ast::ast::ModuleExportName::StringLiteral(lit) => lit.value.to_string(),
        };

        return Some(format!("export {{ {mapped_name} as {exported_name} }}"));
      }
    }

    Some(Self::extract_text(source, export.span()))
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

      if let Some(oxc::ast::ast::ImportDeclarationSpecifier::ImportSpecifier(spec)) =
        specifiers.first()
      {
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

  fn extract_text(source: &str, span: oxc::span::Span) -> String {
    let start = span.start as usize;
    let end = span.end as usize;
    if start < source.len() && end <= source.len() && start < end {
      source[start..end].to_string()
    } else {
      String::new()
    }
  }
}
