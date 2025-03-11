use oxc::{
  ast::{
    AstKind,
    ast::{IdentifierReference, UnaryOperator},
  },
  semantic::{SymbolFlags, SymbolId},
  span::Span,
};
use rolldown_common::{Specifier, SymbolRef};
use rolldown_error::BuildDiagnostic;

use super::AstScanner;

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  pub fn check_import_assign(&mut self, ident: &IdentifierReference, symbol_id: SymbolId) {
    let symbol_flag = self.result.symbol_ref_db.symbol_flags(symbol_id);
    if symbol_flag.contains(SymbolFlags::Import) {
      let symbol_ref: SymbolRef = (self.idx, symbol_id).into();
      let is_namespace = self
        .result
        .named_imports
        .get(&symbol_ref)
        .is_some_and(|import| matches!(import.imported, Specifier::Star));
      if is_namespace {
        if let Some((span, name)) = self.get_span_if_namespace_specifier_updated() {
          self.result.errors.push(BuildDiagnostic::assign_to_import(
            self.id.resource_id().clone(),
            self.source.clone(),
            span,
            name.into(),
          ));
          return;
        }
      }
      let reference_flag = self.result.symbol_ref_db.get_reference(ident.reference_id()).flags();
      if reference_flag.is_write() {
        self.result.errors.push(BuildDiagnostic::assign_to_import(
          self.id.resource_id().clone(),
          self.source.clone(),
          ident.span,
          ident.name.as_str().into(),
        ));
      }
    }
  }

  pub fn get_span_if_namespace_specifier_updated(&self) -> Option<(Span, &'ast str)> {
    let ancestor_cursor = self.visit_path.len() - 1;
    let parent_node = self.visit_path.get(ancestor_cursor)?;
    if let AstKind::MemberExpression(expr) = parent_node {
      let parent_parent_node = self.visit_path.get(ancestor_cursor - 1)?;
      let is_unary_expression_with_delete_operator = |kind| matches!(kind, AstKind::UnaryExpression(expr) if expr.operator == UnaryOperator::Delete);
      let parent_parent_kind = *parent_parent_node;
      if matches!(parent_parent_kind, AstKind::SimpleAssignmentTarget(_))
        // delete namespace.module
        || is_unary_expression_with_delete_operator(parent_parent_kind)
        // delete namespace?.module
        || matches!(parent_parent_kind, AstKind::ChainExpression(_) if self.visit_path.get(ancestor_cursor - 2).is_some_and(|item| {
          is_unary_expression_with_delete_operator(*item)
        }))
      {
        return expr.static_property_info();
      }
    }
    None
  }
}
