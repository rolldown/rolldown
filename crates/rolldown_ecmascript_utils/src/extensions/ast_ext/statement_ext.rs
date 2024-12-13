use oxc::ast::ast;

pub trait StatementExt<'me, 'ast> {
  fn is_import_declaration(&self) -> bool;
  fn as_import_declaration(&'me self) -> Option<&'me ast::ImportDeclaration<'ast>>;
  fn as_export_default_declaration_mut(
    &'me mut self,
  ) -> Option<&'me mut ast::ExportDefaultDeclaration<'ast>>;
  fn as_export_all_declaration(&self) -> Option<&ast::ExportAllDeclaration<'ast>>;
  fn as_export_named_declaration(&self) -> Option<&ast::ExportNamedDeclaration<'ast>>;
  fn as_export_named_declaration_mut(&mut self) -> Option<&mut ast::ExportNamedDeclaration<'ast>>;

  fn is_function_declaration(&self) -> bool;
  fn as_function_declaration(&self) -> Option<&ast::Function<'ast>>;

  fn is_module_declaration_with_source(&self) -> bool;
}

impl<'ast> StatementExt<'_, 'ast> for ast::Statement<'ast> {
  fn is_import_declaration(&self) -> bool {
    matches!(self, ast::Statement::ImportDeclaration(_))
  }

  fn as_import_declaration(&self) -> Option<&ast::ImportDeclaration<'ast>> {
    if let ast::Statement::ImportDeclaration(import_decl) = self {
      return Some(&**import_decl);
    }
    None
  }

  fn as_export_default_declaration_mut(
    &mut self,
  ) -> Option<&mut ast::ExportDefaultDeclaration<'ast>> {
    if let ast::Statement::ExportDefaultDeclaration(export_default_decl) = self {
      return Some(&mut **export_default_decl);
    }
    None
  }

  fn as_export_all_declaration(&self) -> Option<&ast::ExportAllDeclaration<'ast>> {
    if let ast::Statement::ExportAllDeclaration(export_all_decl) = self {
      return Some(&**export_all_decl);
    }
    None
  }

  fn as_export_named_declaration(&self) -> Option<&ast::ExportNamedDeclaration<'ast>> {
    if let ast::Statement::ExportNamedDeclaration(export_named_decl) = self {
      return Some(&**export_named_decl);
    }
    None
  }

  fn as_export_named_declaration_mut(&mut self) -> Option<&mut ast::ExportNamedDeclaration<'ast>> {
    if let ast::Statement::ExportNamedDeclaration(export_named_decl) = self {
      return Some(&mut **export_named_decl);
    }
    None
  }

  fn as_function_declaration(&self) -> Option<&ast::Function<'ast>> {
    if let ast::Statement::FunctionDeclaration(func_decl) = self {
      Some(func_decl)
    } else {
      None
    }
  }

  fn is_function_declaration(&self) -> bool {
    self.as_function_declaration().is_some()
  }

  /// Check if the statement is `[import|export] ... from ...` or `export ... from ...`
  fn is_module_declaration_with_source(&self) -> bool {
    matches!(self.as_module_declaration(), Some(decl) if decl.source().is_some())
  }
}
