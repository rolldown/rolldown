use super::AstScanner;
use oxc::ast::ast;
use oxc_str::CompactStr;
use rolldown_common::{EcmaModuleAstUsage, ImportKind, ImportRecordMeta};
use rolldown_ecmascript_utils::ExpressionExt;
use rustc_hash::{FxHashMap, FxHashSet};

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  pub(crate) fn try_extract_hmr_info_from_hot_accept_call(
    &mut self,
    call_expr: &ast::CallExpression<'ast>,
  ) {
    if !self.immutable_ctx.options.is_dev_mode_enabled() {
      return;
    }
    // Possible call patterns for `import.meta.hot.accept`:
    // - `import.meta.hot.accept()`
    // - `import.meta.hot.accept((newModule) => {})`
    // - `import.meta.hot.accept('./dep.js', ...)`
    // - `import.meta.hot.accept(['./dep1.js', './dep2.js'], ...)`

    if call_expr.callee.is_import_meta_hot_accept_exports() {
      self.extract_hmr_accept_exports(call_expr);
      return;
    }
    // Check whether the callee is `import.meta.hot.accept`.
    if !call_expr.callee.is_import_meta_hot_accept() {
      return;
    }

    let mut module_request_to_import_record_idx = FxHashMap::default();

    match call_expr.arguments.as_slice() {
      // `import.meta.hot.accept()`
      // `import.meta.hot.accept(<any expression>)`
      [] | [_] => {
        self.result.ast_usage.insert(EcmaModuleAstUsage::HmrSelfAccept);
      }
      // `import.meta.hot.accept('./dep.js', <any expression>)`
      [ast::Argument::StringLiteral(string_literal), _] => {
        module_request_to_import_record_idx.insert(
          string_literal.value.as_str().into(),
          self.add_import_record(
            &string_literal.value,
            ImportKind::HotAccept,
            string_literal.span,
            call_expr.span,
            ImportRecordMeta::empty(),
            None,
          ),
        );
      }
      // `import.meta.hot.accept(['./dep1.js', './dep2.js'], <any expression>)`
      [ast::Argument::ArrayExpression(array_expression), _] => {
        module_request_to_import_record_idx.extend(
          array_expression
            .elements
            .iter()
            .filter_map(|element| {
              if let ast::ArrayExpressionElement::StringLiteral(string_literal) = element {
                Some((string_literal.value, string_literal.span))
              } else {
                None
              }
            })
            .map(|(lit, span)| {
              (
                lit.as_str().into(),
                self.add_import_record(
                  &lit,
                  ImportKind::HotAccept,
                  span,
                  call_expr.span,
                  ImportRecordMeta::empty(),
                  None,
                ),
              )
            }),
        );
      }
      _ => {
        // TODO(hyf0): Unsupported call pattern, maybe we should raise a warning here?
      }
    }

    self
      .result
      .hmr_info
      .module_request_to_import_record_idx
      .extend(module_request_to_import_record_idx);
  }

  /// `import.meta.hot.acceptExports('a' | ['a', 'b'], cb?)`. The names are only a hint for
  /// the server-side prediction and for shipping import bindings; the client records the
  /// real call at runtime.
  fn extract_hmr_accept_exports(&mut self, call_expr: &ast::CallExpression<'ast>) {
    let seen_before = self.result.ast_usage.contains(EcmaModuleAstUsage::HmrAcceptExports);
    self.result.ast_usage.insert(EcmaModuleAstUsage::HmrAcceptExports);
    if seen_before && self.result.hmr_info.accepted_exports.is_none() {
      // an earlier call was unreadable, so the set stays unknown
      return;
    }
    let names: Option<FxHashSet<CompactStr>> = match call_expr.arguments.first() {
      Some(ast::Argument::StringLiteral(lit)) => {
        Some(std::iter::once(lit.value.as_str().into()).collect())
      }
      Some(ast::Argument::ArrayExpression(array)) => array
        .elements
        .iter()
        .map(|element| match element {
          ast::ArrayExpressionElement::StringLiteral(lit) => Some(lit.value.as_str().into()),
          _ => None,
        })
        .collect(),
      _ => None,
    };
    match (&mut self.result.hmr_info.accepted_exports, names) {
      (Some(existing), Some(names)) => existing.extend(names),
      (existing, names) => *existing = names,
    }
  }
}
