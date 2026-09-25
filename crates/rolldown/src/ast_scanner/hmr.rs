use super::AstScanner;
use oxc::{
  ast::ast,
  span::{GetSpan, Span},
};
use rolldown_common::{EcmaModuleAstUsage, ImportKind, ImportRecordMeta};
use rolldown_ecmascript_utils::ExpressionExt;
use rolldown_error::BuildDiagnostic;

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  /// Follows Vite's `lexAcceptedHmrDeps`: any first argument that is not a static string or
  /// an array makes the module self-accepting, and a dep that is not a static string is an error.
  pub(crate) fn try_extract_hmr_info_from_hot_accept_call(
    &mut self,
    call_expr: &ast::CallExpression<'ast>,
  ) {
    if !self.immutable_ctx.options.is_dev_mode_enabled() {
      return;
    }
    if !call_expr.callee.is_import_meta_hot_accept() {
      return;
    }

    match call_expr.arguments.first() {
      Some(ast::Argument::ArrayExpression(array_expression)) => {
        for element in &array_expression.elements {
          match element.as_expression() {
            Some(expr) if expr.as_static_module_request().is_some() => {
              self.add_hot_accept_dep(expr, call_expr);
            }
            _ if element.is_elision() => {}
            _ => self.report_non_static_hot_accept_dep(element.span()),
          }
        }
      }
      Some(argument) => match argument.as_expression() {
        Some(expr) if expr.as_static_module_request().is_some() => {
          self.add_hot_accept_dep(expr, call_expr);
        }
        Some(ast::Expression::TemplateLiteral(template)) => {
          self.report_non_static_hot_accept_dep(template.span);
        }
        _ => {
          self.result.ast_usage.insert(EcmaModuleAstUsage::HmrSelfAccept);
        }
      },
      None => {
        self.result.ast_usage.insert(EcmaModuleAstUsage::HmrSelfAccept);
      }
    }
  }

  fn report_non_static_hot_accept_dep(&mut self, span: Span) {
    self.result.errors.push(BuildDiagnostic::unsupported_feature(
      self.immutable_ctx.id.as_arc_str().clone(),
      self.immutable_ctx.source.clone(),
      span,
      "`import.meta.hot.accept()` can only accept string literals or an array of string literals."
        .to_string(),
    ));
  }

  fn add_hot_accept_dep(
    &mut self,
    expr: &ast::Expression<'ast>,
    call_expr: &ast::CallExpression<'ast>,
  ) {
    let Some(request) = expr.as_static_module_request() else { return };
    let record_idx = self.add_import_record(
      &request,
      ImportKind::HotAccept,
      expr.span(),
      call_expr.span,
      ImportRecordMeta::empty(),
      None,
    );
    self
      .result
      .hmr_info
      .module_request_to_import_record_idx
      .insert(request.as_str().into(), record_idx);
  }
}
