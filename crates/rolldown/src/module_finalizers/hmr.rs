use oxc::{
  ast::ast::{self},
  span::SPAN,
};
use rolldown_ecmascript_utils::ExpressionExt;

use crate::hmr::utils::HmrAstBuilder;

use super::ScopeHoistingFinalizer;

impl<'ast> ScopeHoistingFinalizer<'_, 'ast> {
  pub fn generate_hmr_header(&self) -> Vec<ast::Statement<'ast>> {
    let mut ret = vec![];
    if !self.ctx.options.is_hmr_enabled()
      || self.ctx.module.id.as_ref() == rolldown_plugin_hmr::HMR_RUNTIME_MODULE_SPECIFIER
    {
      return ret;
    }

    // `var $hot = __rolldown_runtime__.createModuleHotContext(moduleId);`
    ret.push(self.create_module_hot_context_initializer_stmt());

    ret.push(self.create_register_module_stmt());

    ret
  }

  pub fn rewrite_import_meta_hot(&self, expr: &mut ast::Expression<'ast>) {
    if expr.is_import_meta_hot() {
      if let Some(hmr_hot_ref) = self.ctx.module.ecma_view.hmr_hot_ref {
        let hot_name = self.canonical_name_for(hmr_hot_ref);
        *expr = self.snippet.id_ref_expr(hot_name, SPAN);
      }
    }
  }

  pub fn rewrite_hot_accept_call_deps(&self, call_expr: &mut ast::CallExpression<'ast>) {
    if !self.ctx.options.is_hmr_enabled() {
      return;
    }

    // Check whether the callee is `import.meta.hot.accept`.
    if !call_expr.callee.is_import_meta_hot_accept() {
      return;
    }

    if call_expr.arguments.is_empty() {
      // `import.meta.hot.accept()`
      return;
    }

    match &mut call_expr.arguments[0] {
      ast::Argument::StringLiteral(string_literal) => {
        // `import.meta.hot.accept('./dep.js', ...)`
        let import_record = &self.ctx.module.import_records[self
          .ctx
          .module
          .hmr_info
          .module_request_to_import_record_idx[string_literal.value.as_str()]];
        string_literal.value =
          self.snippet.builder.atom(self.ctx.modules[import_record.resolved_module].stable_id());
      }
      ast::Argument::ArrayExpression(array_expression) => {
        // `import.meta.hot.accept(['./dep1.js', './dep2.js'], ...)`
        array_expression.elements.iter_mut().for_each(|element| {
          if let ast::ArrayExpressionElement::StringLiteral(string_literal) = element {
            let import_record = &self.ctx.module.import_records[self
              .ctx
              .module
              .hmr_info
              .module_request_to_import_record_idx[string_literal.value.as_str()]];
            string_literal.value = self
              .snippet
              .builder
              .atom(self.ctx.modules[import_record.resolved_module].stable_id());
          }
        });
      }
      _ => {}
    }
  }
}
