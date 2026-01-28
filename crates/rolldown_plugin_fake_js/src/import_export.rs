use oxc::ast::ast::{
  ExportAllDeclaration, ExportDefaultDeclaration, ExportNamedDeclaration, Expression,
  ImportDeclaration, Statement, TSExportAssignment, TSImportEqualsDeclaration,
};
use oxc::span::GetSpan;

pub struct ImportExportRewriter;

impl ImportExportRewriter {
  pub fn rewrite_statement(
    stmt: &Statement,
    source: &str,
    type_only_ids: &mut Vec<String>,
  ) -> Option<String> {
    match stmt {
      Statement::ImportDeclaration(import) => {
        Self::rewrite_import_declaration(import, source, type_only_ids)
      }
      Statement::ExportNamedDeclaration(export) => {
        Self::rewrite_export_named_declaration(export, source, type_only_ids)
      }
      Statement::ExportAllDeclaration(export) => {
        Self::rewrite_export_all_declaration(export, source)
      }
      Statement::ExportDefaultDeclaration(export) => {
        Self::rewrite_export_default_declaration(export, source)
      }
      Statement::TSImportEqualsDeclaration(import_eq) => {
        Self::rewrite_ts_import_equals(import_eq, source)
      }
      Statement::TSExportAssignment(export_assign) => {
        Self::rewrite_ts_export_assignment(export_assign, source)
      }
      _ => None,
    }
  }

  #[expect(clippy::unnecessary_wraps)]
  fn rewrite_import_declaration(
    import: &ImportDeclaration,
    source: &str,
    type_only_ids: &mut Vec<String>,
  ) -> Option<String> {
    let import_text = Self::extract_text(source, import.span());

    if import.import_kind == oxc::ast::ast::ImportOrExportKind::Type {
      if let Some(specifiers) = &import.specifiers {
        for specifier in specifiers {
          match specifier {
            oxc::ast::ast::ImportDeclarationSpecifier::ImportSpecifier(spec) => {
              type_only_ids.push(spec.local.name.to_string());
            }
            oxc::ast::ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(spec) => {
              type_only_ids.push(spec.local.name.to_string());
            }
            oxc::ast::ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(spec) => {
              type_only_ids.push(spec.local.name.to_string());
            }
          }
        }
      }
    }

    if let Some(specifiers) = &import.specifiers {
      for specifier in specifiers {
        if let oxc::ast::ast::ImportDeclarationSpecifier::ImportSpecifier(spec) = specifier {
          if spec.import_kind == oxc::ast::ast::ImportOrExportKind::Type {
            type_only_ids.push(spec.local.name.to_string());
          }
        }
      }
    }

    Some(import_text)
  }

  fn rewrite_export_named_declaration(
    export: &ExportNamedDeclaration,
    source: &str,
    type_only_ids: &mut Vec<String>,
  ) -> Option<String> {
    if export.declaration.is_some() {
      return None;
    }

    let export_text = Self::extract_text(source, export.span());

    if export.export_kind == oxc::ast::ast::ImportOrExportKind::Type {
      for specifier in &export.specifiers {
        let exported_name = match &specifier.exported {
          oxc::ast::ast::ModuleExportName::IdentifierName(id) => id.name.to_string(),
          oxc::ast::ast::ModuleExportName::IdentifierReference(id) => id.name.to_string(),
          oxc::ast::ast::ModuleExportName::StringLiteral(lit) => lit.value.to_string(),
        };
        type_only_ids.push(exported_name);
      }
    }

    for specifier in &export.specifiers {
      if specifier.export_kind == oxc::ast::ast::ImportOrExportKind::Type {
        let exported_name = match &specifier.exported {
          oxc::ast::ast::ModuleExportName::IdentifierName(id) => id.name.to_string(),
          oxc::ast::ast::ModuleExportName::IdentifierReference(id) => id.name.to_string(),
          oxc::ast::ast::ModuleExportName::StringLiteral(lit) => lit.value.to_string(),
        };
        type_only_ids.push(exported_name);
      }
    }

    Some(export_text)
  }

  #[expect(clippy::unnecessary_wraps)]
  fn rewrite_export_all_declaration(export: &ExportAllDeclaration, source: &str) -> Option<String> {
    Some(Self::extract_text(source, export.span()))
  }

  fn rewrite_export_default_declaration(
    export: &ExportDefaultDeclaration,
    _source: &str,
  ) -> Option<String> {
    if let oxc::ast::ast::ExportDefaultDeclarationKind::Identifier(id) = &export.declaration {
      return Some(format!("export {{ {} as default }}", id.name));
    }

    None
  }

  #[expect(clippy::unnecessary_wraps)]
  fn rewrite_ts_import_equals(
    import_eq: &TSImportEqualsDeclaration,
    source: &str,
  ) -> Option<String> {
    if let oxc::ast::ast::TSModuleReference::ExternalModuleReference(module_ref) =
      &import_eq.module_reference
    {
      let binding_name = import_eq.id.name.as_str();
      let source_value = &module_ref.expression.value;
      return Some(format!("import {binding_name} from \"{source_value}\""));
    }

    Some(Self::extract_text(source, import_eq.span()))
  }

  fn rewrite_ts_export_assignment(
    export_assign: &TSExportAssignment,
    _source: &str,
  ) -> Option<String> {
    if let Expression::Identifier(id) = &export_assign.expression {
      return Some(format!("export {{ {} as default }}", id.name));
    }

    None
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_extract_text() {
    let code = "hello world";
    let span = oxc::span::Span::new(0, 5);
    let result = ImportExportRewriter::extract_text(code, span);
    assert_eq!(result, "hello");
  }
}
