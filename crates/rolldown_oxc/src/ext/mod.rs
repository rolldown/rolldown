use oxc::allocator::Box;
use oxc::ast::ast;
use oxc::semantic::SymbolId;
use smallvec::SmallVec;
pub trait BindingIdentifierExt {
  fn expect_symbol_id(&self) -> SymbolId;
}

impl BindingIdentifierExt for ast::BindingIdentifier {
  #[inline]
  fn expect_symbol_id(&self) -> SymbolId {
    self.symbol_id.get().unwrap_or_else(|| panic!("fail get symbol id from {self:?}"))
  }
}

pub trait BindingPatternExt {
  fn binding_identifiers(&self) -> smallvec::SmallVec<[&Box<ast::BindingIdentifier>; 1]>;
}

impl BindingPatternExt for ast::BindingPattern<'_> {
  fn binding_identifiers(&self) -> smallvec::SmallVec<[&Box<ast::BindingIdentifier>; 1]> {
    let mut queue = vec![&self.kind];
    let mut ret = SmallVec::default();
    while let Some(binding_kind) = queue.pop() {
      match binding_kind {
        ast::BindingPatternKind::BindingIdentifier(id) => {
          ret.push(id);
        }
        ast::BindingPatternKind::ArrayPattern(arr_pat) => {
          queue.extend(arr_pat.elements.iter().flatten().map(|pat| &pat.kind).rev());
        }
        ast::BindingPatternKind::ObjectPattern(obj_pat) => {
          queue.extend(obj_pat.properties.iter().map(|prop| &prop.value.kind).rev());
        }
        //
        ast::BindingPatternKind::AssignmentPattern(assign_pat) => {
          queue.push(&assign_pat.left.kind);
        }
      };
    }
    ret
  }
}

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
}

impl<'me, 'ast> StatementExt<'me, 'ast> for ast::Statement<'ast> {
  fn is_import_declaration(&self) -> bool {
    matches!(
      self,
      ast::Statement::ModuleDeclaration(module_decl)
        if matches!(module_decl.0, ast::ModuleDeclaration::ImportDeclaration(_))
    )
  }

  fn as_import_declaration(&self) -> Option<&ast::ImportDeclaration<'ast>> {
    if let ast::Statement::ModuleDeclaration(module_decl) = self {
      if let ast::ModuleDeclaration::ImportDeclaration(import_decl) = &module_decl.0 {
        return Some(import_decl);
      }
    }
    None
  }

  fn as_export_default_declaration_mut(
    &mut self,
  ) -> Option<&mut ast::ExportDefaultDeclaration<'ast>> {
    if let ast::Statement::ModuleDeclaration(export_default_decl) = self {
      if let ast::ModuleDeclaration::ExportDefaultDeclaration(export_default_decl) =
        &mut export_default_decl.0
      {
        return Some(export_default_decl);
      }
    }
    None
  }

  fn as_export_all_declaration(&self) -> Option<&ast::ExportAllDeclaration<'ast>> {
    if let ast::Statement::ModuleDeclaration(export_all_decl) = self {
      if let ast::ModuleDeclaration::ExportAllDeclaration(export_all_decl) = &export_all_decl.0 {
        return Some(export_all_decl);
      }
    }
    None
  }

  fn as_export_named_declaration(&self) -> Option<&ast::ExportNamedDeclaration<'ast>> {
    if let ast::Statement::ModuleDeclaration(export_named_decl) = self {
      if let ast::ModuleDeclaration::ExportNamedDeclaration(export_named_decl) =
        &export_named_decl.0
      {
        return Some(export_named_decl);
      }
    }
    None
  }

  fn as_export_named_declaration_mut(&mut self) -> Option<&mut ast::ExportNamedDeclaration<'ast>> {
    if let ast::Statement::ModuleDeclaration(export_named_decl) = self {
      if let ast::ModuleDeclaration::ExportNamedDeclaration(export_named_decl) =
        &mut export_named_decl.0
      {
        return Some(export_named_decl);
      }
    }
    None
  }

  fn as_function_declaration(&self) -> Option<&ast::Function<'ast>> {
    if let ast::Statement::Declaration(ast::Declaration::FunctionDeclaration(func_decl)) = self {
      Some(func_decl)
    } else {
      None
    }
  }

  fn is_function_declaration(&self) -> bool {
    self.as_function_declaration().is_some()
  }
}

pub trait ExpressionExt<'ast> {
  fn as_call_expression(&self) -> Option<&ast::CallExpression<'ast>>;
}

impl<'ast> ExpressionExt<'ast> for ast::Expression<'ast> {
  fn as_call_expression(&self) -> Option<&ast::CallExpression<'ast>> {
    if let ast::Expression::CallExpression(call_expr) = self {
      Some(call_expr)
    } else {
      None
    }
  }
}
