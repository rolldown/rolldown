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

pub trait StatementExt {
  fn is_import_declaration(&self) -> bool;
  fn as_import_declaration(&self) -> Option<&ast::ImportDeclaration<'_>>;
}

impl StatementExt for ast::Statement<'_> {
  fn is_import_declaration(&self) -> bool {
    matches!(
      self,
      ast::Statement::ModuleDeclaration(module_decl)
        if matches!(module_decl.0, ast::ModuleDeclaration::ImportDeclaration(_))
    )
  }

  fn as_import_declaration(&self) -> Option<&ast::ImportDeclaration<'_>> {
    if let ast::Statement::ModuleDeclaration(module_decl) = self {
      if let ast::ModuleDeclaration::ImportDeclaration(import_decl) = &module_decl.0 {
        return Some(import_decl);
      }
    }
    None
  }
}
