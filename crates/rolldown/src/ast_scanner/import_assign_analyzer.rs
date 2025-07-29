use oxc::{
  ast::{
    AstKind, MemberExpressionKind,
    ast::{Expression, IdentifierReference, UnaryOperator},
  },
  semantic::{SymbolFlags, SymbolId},
  span::Span,
};
use rolldown_common::{Specifier, SymbolRef};
use rolldown_error::BuildDiagnostic;

use super::AstScanner;

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  pub fn check_import_assign(&mut self, ident: &IdentifierReference, symbol_id: SymbolId) {
    let symbol_flag = self.result.symbol_ref_db.scoping().symbol_flags(symbol_id);
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
      let reference_flag =
        self.result.symbol_ref_db.scoping().get_reference(ident.reference_id()).flags();
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
    if let Some(member_expr_kind) = parent_node.as_member_expression_kind() {
      let parent_parent_kind = self.visit_path.get(ancestor_cursor - 1)?;
      let is_unary_expression_with_delete_operator = |kind: &AstKind| matches!(kind, AstKind::UnaryExpression(expr) if expr.operator == UnaryOperator::Delete);
      if member_expr_kind.is_assigned_to_in_parent(parent_parent_kind)
        // delete namespace.module
        || is_unary_expression_with_delete_operator(parent_parent_kind)
        // delete namespace?.module
        || matches!(parent_parent_kind, AstKind::ChainExpression(_) if self.visit_path.get(ancestor_cursor - 2).is_some_and(|item| {
          is_unary_expression_with_delete_operator(item)
        }))
      {
        return match member_expr_kind {
          MemberExpressionKind::Computed(expr) => match &expr.expression {
            Expression::StringLiteral(lit) => Some((lit.span, lit.value.as_str())),
            Expression::TemplateLiteral(lit) => {
              if lit.quasis.len() == 1 {
                lit.quasis[0].value.cooked.map(|cooked| (lit.span, cooked.as_str()))
              } else {
                None
              }
            }
            _ => None,
          },
          MemberExpressionKind::Static(expr) => {
            Some((expr.property.span, expr.property.name.as_str()))
          }
          MemberExpressionKind::PrivateField(_) => None,
        };
      }
    }
    None
  }
}
