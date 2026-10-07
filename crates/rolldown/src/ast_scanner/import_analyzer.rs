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
      let symbol_ref: SymbolRef = (self.immutable_ctx.idx, symbol_id).into();
      let named_import = self.result.named_imports.get(&symbol_ref);
      let import_decl_span = self.result.symbol_ref_db.scoping().symbol_span(symbol_id);
      let is_namespace =
        named_import.is_some_and(|import| matches!(import.imported, Specifier::Star));
      if is_namespace {
        if let Some(parent) = self.visit_path.last() {
          let expr_span = match parent {
            AstKind::CallExpression(call_expr) => {
              (call_expr.callee.node_id() == ident.node_id()).then_some(call_expr.span)
            }
            AstKind::TaggedTemplateExpression(call_expr) => {
              (call_expr.tag.node_id() == ident.node_id()).then_some(call_expr.span)
            }
            _ => None,
          };
          if let Some(span) = expr_span {
            let name = self.result.symbol_ref_db.symbol_name(symbol_id);
            let declaration_span =
              named_import.map(|import| import.span_imported).unwrap_or_default();
            self.result.warnings.push(
              BuildDiagnostic::cannot_call_namespace(
                self.immutable_ctx.id.as_arc_str().clone(),
                self.immutable_ctx.source.clone(),
                span,
                name.into(),
                declaration_span,
              )
              .with_severity_warning(),
            );
          }
        }

        if let Some((span, name)) = self.get_span_if_namespace_specifier_updated() {
          // For namespace imports, get the actual imported name (the namespace identifier)
          let imported_name = self.result.symbol_ref_db.symbol_name(symbol_id);
          self.result.errors.push(BuildDiagnostic::assign_to_import(
            self.immutable_ctx.id.as_arc_str().clone(),
            self.immutable_ctx.source.clone(),
            span,
            name.into(),
            Some(import_decl_span),
            Some(imported_name.into()),
          ));
          return;
        }
      }
      let reference_flag =
        self.result.symbol_ref_db.scoping().get_reference(ident.reference_id()).flags();
      if reference_flag.is_write() {
        self.result.errors.push(BuildDiagnostic::assign_to_import(
          self.immutable_ctx.id.as_arc_str().clone(),
          self.immutable_ctx.source.clone(),
          ident.span,
          ident.name.as_str().into(),
          Some(import_decl_span),
          None, // For non-namespace imports, use the name itself
        ));
      }
    }
  }

  /// Whether the member expression at `visit_path[member_idx]` is the target of an assignment or
  /// a `delete`, not only an object on the way to a deeper target (`ns.a[k] = 1`).
  pub fn is_member_expr_written(&self, member_idx: usize) -> bool {
    let Some(member_expr_kind) =
      self.visit_path.get(member_idx).and_then(AstKind::as_member_expression_kind)
    else {
      return false;
    };
    let ancestor =
      |depth: usize| member_idx.checked_sub(depth).and_then(|i| self.visit_path.get(i));
    let Some(parent_kind) = ancestor(1) else {
      return false;
    };
    let is_unary_expression_with_delete_operator = |kind: &AstKind| matches!(kind, AstKind::UnaryExpression(expr) if expr.operator == UnaryOperator::Delete);
    member_expr_kind.is_assigned_to_in_parent(parent_kind)
      // delete namespace.module
      || is_unary_expression_with_delete_operator(parent_kind)
      // delete namespace?.module
      || matches!(parent_kind, AstKind::ChainExpression(_) if ancestor(2).is_some_and(is_unary_expression_with_delete_operator))
  }

  pub fn get_span_if_namespace_specifier_updated(&self) -> Option<(Span, &'ast str)> {
    let ancestor_cursor = self.visit_path.len() - 1;
    let parent_node = self.visit_path.get(ancestor_cursor)?;
    if let Some(member_expr_kind) = parent_node.as_member_expression_kind() {
      if self.is_member_expr_written(ancestor_cursor) {
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
