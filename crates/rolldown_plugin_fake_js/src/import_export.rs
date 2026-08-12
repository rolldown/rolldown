use oxc::ast::ast::{
  ExportAllDeclaration, ExportDefaultDeclaration, ExportDefaultDeclarationKind,
  ExportFromDeclaration, ExportNamedDeclaration, Expression, ImportDeclaration,
  ImportDeclarationSpecifier, ImportOrExportKind, Statement, TSExportAssignment,
  TSImportEqualsDeclaration, TSModuleReference,
};
use oxc::span::GetSpan;

use crate::codegen;

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
      Statement::ExportFromDeclaration(export) => {
        Self::rewrite_export_from_declaration(export, source, type_only_ids)
      }
      Statement::ExportAllDeclaration(export) => {
        Self::rewrite_export_all_declaration(export, source, type_only_ids)
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
    _type_only_ids: &mut Vec<String>,
  ) -> Option<String> {
    let mut import_text = codegen::extract_span_text(source, import.span());

    // Strip `type` so Rolldown keeps the import. Unlike exports, type-only imports are not tracked.
    if import.import_kind == ImportOrExportKind::Type {
      import_text = import_text.replace("import type ", "import ");
    }

    if let Some(specifiers) = &import.specifiers {
      for specifier in specifiers {
        if let ImportDeclarationSpecifier::ImportSpecifier(spec) = specifier {
          if spec.import_kind == ImportOrExportKind::Type {
            import_text = import_text.replace("{ type ", "{ ");
            import_text = import_text.replace(", type ", ", ");
          }
        }
      }
    }

    Some(import_text)
  }

  #[expect(clippy::unnecessary_wraps)]
  fn rewrite_export_named_declaration(
    export: &ExportNamedDeclaration,
    source: &str,
    type_only_ids: &mut Vec<String>,
  ) -> Option<String> {
    let mut export_text = codegen::extract_span_text(source, export.span());

    // Track by exported name (`as` alias, else local) so render_chunk can restore `type`.
    if export.export_kind == ImportOrExportKind::Type {
      for specifier in &export.specifiers {
        let exported_name = specifier.exported.name().to_string();
        type_only_ids.push(exported_name);
      }
      export_text = export_text.replace("export type {", "export {");
      export_text = export_text.replace("export type*", "export *");
    }

    let mut has_type_specifiers = false;
    for specifier in &export.specifiers {
      if specifier.export_kind == ImportOrExportKind::Type {
        has_type_specifiers = true;
        let exported_name = specifier.exported.name().to_string();
        type_only_ids.push(exported_name);
      }
    }

    if has_type_specifiers {
      export_text = export_text.replace("{ type ", "{ ");
      export_text = export_text.replace(", type ", ", ");
    }

    Some(export_text)
  }

  #[expect(clippy::unnecessary_wraps)]
  fn rewrite_export_from_declaration(
    export: &ExportFromDeclaration,
    source: &str,
    type_only_ids: &mut Vec<String>,
  ) -> Option<String> {
    let mut export_text = codegen::extract_span_text(source, export.span());

    if export.export_kind == ImportOrExportKind::Type {
      for specifier in &export.specifiers {
        let exported_name = specifier.exported.name().to_string();
        type_only_ids.push(exported_name);
      }
      export_text = export_text.replace("export type {", "export {");
      export_text = export_text.replace("export type*", "export *");
    }

    let mut has_type_specifiers = false;
    for specifier in &export.specifiers {
      if specifier.export_kind == ImportOrExportKind::Type {
        has_type_specifiers = true;
        let exported_name = specifier.exported.name().to_string();
        type_only_ids.push(exported_name);
      }
    }

    if has_type_specifiers {
      export_text = export_text.replace("{ type ", "{ ");
      export_text = export_text.replace(", type ", ", ");
    }

    Some(export_text)
  }

  #[expect(clippy::unnecessary_wraps)]
  fn rewrite_export_all_declaration(
    export: &ExportAllDeclaration,
    source: &str,
    type_only_ids: &mut Vec<String>,
  ) -> Option<String> {
    let mut export_text = codegen::extract_span_text(source, export.span());

    if export.export_kind == ImportOrExportKind::Type {
      if let Some(exported) = &export.exported {
        let exported_name = exported.name().to_string();
        type_only_ids.push(exported_name);
      }
      export_text = export_text.replace("export type *", "export *");
    }

    Some(export_text)
  }

  fn rewrite_export_default_declaration(
    export: &ExportDefaultDeclaration,
    _source: &str,
  ) -> Option<String> {
    if let ExportDefaultDeclarationKind::Identifier(id) = &export.declaration {
      return Some(format!("export {{ {} as default }}", id.name));
    }

    None
  }

  #[expect(clippy::unnecessary_wraps)]
  fn rewrite_ts_import_equals(
    import_eq: &TSImportEqualsDeclaration,
    source: &str,
  ) -> Option<String> {
    if let TSModuleReference::ExternalModuleReference(module_ref) = &import_eq.module_reference {
      let binding_name = import_eq.id.name.as_str();
      let source_value = &module_ref.expression.value;
      return Some(format!("import {binding_name} from \"{source_value}\""));
    }

    Some(codegen::extract_span_text(source, import_eq.span()))
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
}
