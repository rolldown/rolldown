use oxc::{
  ast::ast::{
    ExportFromDeclaration, ImportDeclaration, ImportDeclarationSpecifier, ImportOrExportKind,
  },
  ast_visit::VisitJsMut,
  str::JSStr,
};

pub struct TypeImportVisitor<'ast> {
  pub imported: Vec<JSStr<'ast>>,
}

impl<'ast> VisitJsMut<'ast> for TypeImportVisitor<'ast> {
  fn visit_import_declaration(&mut self, decl: &mut ImportDeclaration<'ast>) {
    match decl.import_kind {
      ImportOrExportKind::Type => {
        self.imported.push(decl.source.value);
      }
      ImportOrExportKind::Value => {
        if let Some(specifiers) = &decl.specifiers {
          for specifier in specifiers {
            if let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier {
              if matches!(specifier.import_kind, ImportOrExportKind::Type) {
                self.imported.push(decl.source.value);
                break;
              }
            }
          }
        }
      }
    }
  }

  fn visit_export_from_declaration(&mut self, decl: &mut ExportFromDeclaration<'ast>) {
    match decl.export_kind {
      ImportOrExportKind::Type => {
        self.imported.push(decl.source.value);
      }
      ImportOrExportKind::Value => {
        for specifier in &decl.specifiers {
          if matches!(specifier.export_kind, ImportOrExportKind::Type) {
            self.imported.push(decl.source.value);
            break;
          }
        }
      }
    }
  }
}
