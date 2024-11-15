use oxc::ast::ast::NewExpression;
use rolldown_common::{ImportKind, ImportRecordMeta, ModuleType, ROLLDOWN_IGNORE};
use rolldown_ecmascript_utils::ExpressionExt;

use super::AstScanner;

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  /// Handle exact `new URL('path', import.meta.url)` pattern
  pub fn handle_new_url_with_string_literal_and_import_meta_url(
    &mut self,
    expr: &NewExpression<'ast>,
  ) {
    let is_callee_global_url = matches!(expr.callee.as_identifier(), Some(ident) if ident.name == "URL" && self.is_global_identifier_reference(ident));

    if !is_callee_global_url {
      return;
    }

    let is_second_arg_import_meta_url = expr
      .arguments
      .get(1)
      .map_or(false, |arg| arg.as_expression().is_some_and(ExpressionExt::is_import_meta_url));

    if !is_second_arg_import_meta_url {
      return;
    }

    let Some(first_arg_string_literal) = expr
      .arguments
      .first()
      .and_then(|arg| arg.as_expression().and_then(|expr| expr.as_string_literal()))
    else {
      return;
    };

    let has_leading_ignore_comment = self
      .comments
      .binary_search_by(|c| c.attached_to.cmp(&first_arg_string_literal.span.start))
      .ok()
      .and_then(|i| {
        let comment = &self.comments[i];
        let original_source = &self.source.as_str()[comment.content_span()];
        original_source.contains(ROLLDOWN_IGNORE).then_some(())
      })
      .is_some();
    if has_leading_ignore_comment {
      return;
    }
    let path = &first_arg_string_literal.value;

    if path.starts_with("data:") {
      return;
    }

    let idx =
      self.add_import_record(path, ImportKind::NewUrl, expr.span, ImportRecordMeta::empty());
    self.result.import_records[idx].asserted_module_type = Some(ModuleType::Asset);
    self.result.new_url_references.insert(expr.span, idx);
  }
}
