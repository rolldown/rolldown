use oxc::{
  ast::ast::Argument,
  ast_visit::{VisitMut, walk_mut::walk_new_expression},
};
use rolldown_ecmascript_utils::ExpressionExt;

pub struct NewUrlVisitor {
  pub urls: Vec<(String, oxc::span::Span)>,
}

impl VisitMut<'_> for NewUrlVisitor {
  fn visit_new_expression(&mut self, it: &mut oxc::ast::ast::NewExpression<'_>) {
    if it.callee.is_specific_id("URL") && it.arguments.len() == 2 {
      if !it.arguments[1].as_expression().is_some_and(ExpressionExt::is_import_meta_url) {
        return;
      }

      let (url, span) = match &it.arguments[0] {
        Argument::StringLiteral(lit) => (lit.value.to_string(), lit.span),
        Argument::TemplateLiteral(template) => {
          if let Some(lit) = template.single_quasi() {
            (lit.to_string(), template.span)
          } else {
            todo!()
          }
        }
        _ => return,
      };

      self.urls.push((url, span));
    }
    walk_new_expression(self, it);
  }
}
