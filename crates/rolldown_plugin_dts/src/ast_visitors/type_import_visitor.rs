use oxc::{
  ast::ast::{
    ExportNamedDeclaration, ImportDeclaration, ImportDeclarationSpecifier, ImportOrExportKind,
  },
  ast_visit::VisitMut,
  span::Atom,
};

pub struct TypeImportVisitor<'ast> {
  pub imported: Vec<Atom<'ast>>,
}

impl<'ast> VisitMut<'ast> for TypeImportVisitor<'ast> {
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

  fn visit_export_named_declaration(&mut self, decl: &mut ExportNamedDeclaration<'ast>) {
    if let Some(source) = &decl.source {
      match decl.export_kind {
        ImportOrExportKind::Type => {
          self.imported.push(source.value);
        }
        ImportOrExportKind::Value => {
          for specifier in &decl.specifiers {
            if matches!(specifier.export_kind, ImportOrExportKind::Type) {
              self.imported.push(source.value);
              break;
            }
          }
        }
      }
    }
  }
}
