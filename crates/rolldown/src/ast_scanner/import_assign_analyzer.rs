use oxc::{
  ast::{
    ast::{IdentifierReference, UnaryOperator},
    AstKind,
  },
  semantic::{SymbolFlags, SymbolId},
};
use rolldown_error::BuildDiagnostic;

use super::AstScanner;

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  pub fn check_import_assign(&mut self, ident: &IdentifierReference, symbol_id: SymbolId) {
    let symbol_flag = self.result.symbol_ref_db.get_flags(symbol_id);
    if symbol_flag.contains(SymbolFlags::Import) {
      let reference_flag = self.scopes.references[ident.reference_id()].flags();
      if reference_flag.is_write() {
        self.result.errors.push(BuildDiagnostic::assign_to_import(
          self.file_path.inner().clone(),
          self.source.clone(),
          ident.span,
          ident.name.as_str().into(),
        ));
      }
    }
    // TODO: namespace
  }

  #[allow(unused)]
  pub fn is_namespace_specifier_updated(&mut self, ident: &IdentifierReference) -> bool {
    let ancestor_cursor = self.visit_path.len() - 1;
    let Some(parent_node) = self.visit_path.get(ancestor_cursor) else {
      return false;
    };
    if let AstKind::MemberExpression(expr) = parent_node {
      let Some(parent_parent_node) = self.visit_path.get(ancestor_cursor - 1) else {
        return false;
      };
      let is_unary_expression_with_delete_operator = |kind| matches!(kind, AstKind::UnaryExpression(expr) if expr.operator == UnaryOperator::Delete);
      let parent_parent_kind = *parent_parent_node;
      if matches!(parent_parent_kind, AstKind::SimpleAssignmentTarget(_))
                            // delete namespace.module
                            || is_unary_expression_with_delete_operator(parent_parent_kind)
                            // delete namespace?.module
                            || matches!(parent_parent_kind, AstKind::ChainExpression(_) if self.visit_path.get(ancestor_cursor - 2).is_some_and(|item| {
                              is_unary_expression_with_delete_operator(*item)
                            }))
      {}
    }
    false
  }
}
