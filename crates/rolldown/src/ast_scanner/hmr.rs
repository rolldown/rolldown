use super::AstScanner;
use oxc::ast::ast;
use rolldown_common::{EcmaModuleAstUsage, ImportKind, ImportRecordMeta};
use rolldown_ecmascript_utils::ExpressionExt;

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  pub(crate) fn try_extract_hmr_info_from_hot_accept_call(
    &mut self,
    call_expr: &ast::CallExpression<'ast>,
  ) {
    if !self.options.is_hmr_enabled() {
      return;
    }

    // Check whether the callee is `import.meta.hot.accept`.
    if !call_expr.callee.is_import_meta_hot_accept() {
      return;
    }

    // Possible call patterns for `import.meta.hot.accept`:
    // - `import.meta.hot.accept()`
    // - `import.meta.hot.accept((newModule) => {})`
    // - `import.meta.hot.accept('./dep.js', ...)`
    // - `import.meta.hot.accept(['./dep1.js', './dep2.js'], ...)`

    if call_expr.arguments.len() == 0 {
      // `import.meta.hot.accept()`
      self.ast_usage.insert(EcmaModuleAstUsage::HmrSelfAccept);
      return;
    }

    let mut hmr_deps = vec![];
    match &call_expr.arguments[0] {
      ast::Argument::StringLiteral(string_literal) => {
        // `import.meta.hot.accept('./dep.js', ...)`
        hmr_deps.push(self.add_import_record(
          &string_literal.value,
          ImportKind::HotAccept,
          call_expr.span,
          ImportRecordMeta::empty(),
        ));
      }
      ast::Argument::ArrayExpression(array_expression) => {
        // `import.meta.hot.accept(['./dep1.js', './dep2.js'], ...)`
        hmr_deps.extend(
          array_expression
            .elements
            .iter()
            .filter_map(|element| {
              if let ast::ArrayExpressionElement::StringLiteral(string_literal) = element {
                Some(string_literal.value)
              } else {
                None
              }
            })
            .map(|lit| {
              self.add_import_record(
                &lit,
                ImportKind::HotAccept,
                call_expr.span,
                ImportRecordMeta::empty(),
              )
            }),
        );
      }
      _ => {}
    }
    self.ast_usage.insert(EcmaModuleAstUsage::HmrSelfAccept);
  }
}
