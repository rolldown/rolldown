use std::ops::Range;

use oxc::ast_visit::{Visit, walk};

pub struct ScriptInlineImportVisitor<'a> {
  pub offset: usize,
  pub script_urls: &'a mut Vec<(String, Range<usize>)>,
}

impl Visit<'_> for ScriptInlineImportVisitor<'_> {
  fn visit_import_expression(&mut self, it: &oxc::ast::ast::ImportExpression<'_>) {
    if let oxc::ast::ast::Expression::StringLiteral(lit) = &it.source {
      self.script_urls.push((
        lit.value.to_string(),
        lit.span.start as usize + self.offset + 1..lit.span.end as usize + self.offset - 1,
      ));
      return;
    }
    walk::walk_import_expression(self, it);
  }
}
