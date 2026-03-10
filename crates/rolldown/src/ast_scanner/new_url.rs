use oxc::ast::{
  Comment,
  ast::{Expression, NewExpression},
};
use rolldown_common::{ImportKind, ImportRecordMeta, ModuleType, get_leading_comment};
use rolldown_ecmascript_utils::ExpressionExt;
use rolldown_utils::dataurl::is_data_url;

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
      .is_some_and(|arg| arg.as_expression().is_some_and(ExpressionExt::is_import_meta_url));

    if !is_second_arg_import_meta_url {
      return;
    }

    let Some(first_arg) = expr.arguments.first().and_then(|arg| arg.as_expression()) else {
      return;
    };

    let (path, first_arg_span) = match first_arg {
      Expression::StringLiteral(lit) => (&lit.value, lit.span),
      Expression::TemplateLiteral(tpl) if tpl.is_no_substitution_template() => {
        let Some(value) = &tpl.quasis[0].value.cooked else {
          return;
        };
        (value, tpl.span)
      }
      _ => return,
    };

    let has_leading_ignore_comment = get_leading_comment(
      self.immutable_ctx.comments,
      first_arg_span,
      Some(|comment: &Comment| comment.is_vite()),
    )
    .is_some();
    if has_leading_ignore_comment {
      return;
    }

    if is_data_url(path) {
      return;
    }

    let idx = self.add_import_record(
      path,
      ImportKind::NewUrl,
      first_arg_span,
      ImportRecordMeta::empty(),
      None,
    );
    self.result.import_records[idx].asserted_module_type = Some(ModuleType::Asset);
    self.result.new_url_references.insert(expr.span, idx);
  }
}
