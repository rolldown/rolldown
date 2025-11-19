use oxc::{
  ast::ast::Argument,
  ast_visit::{VisitMut, walk_mut::walk_new_expression},
};
use rolldown_ecmascript_utils::ExpressionExt;
use rolldown_plugin::{LogWithoutPlugin, PluginContext};
use rolldown_plugin_utils::inject_query;

pub struct NewUrlVisitor<'a, 'b> {
  pub urls: Vec<(String, oxc::span::Span)>,
  pub s: &'a mut Option<string_wizard::MagicString<'b>>,
  pub code: &'b str,
  pub ctx: &'a PluginContext,
}

impl VisitMut<'_> for NewUrlVisitor<'_, '_> {
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
            // TODO: Escape glob syntax in template literals
            let glob = match super::ast_utils::template_literal_to_glob(template) {
              Ok(glob) => glob,
              Err(error) => {
                self
                  .ctx
                  .warn(LogWithoutPlugin { message: error.to_string(), ..Default::default() });
                return;
              }
            };

            // Validate that the URL starts with a relative path
            if !glob.starts_with("./") && !glob.starts_with("../") {
              self.ctx.warn(LogWithoutPlugin {
                message: format!(
                  "new URL() with import.meta.url must use a relative path. Original: {}, Generated glob: `{}`",
                  template.span.source_text(self.code),
                  glob
                ),
                ..Default::default()
              });
              return;
            }

            let s = self.s.get_or_insert_with(|| string_wizard::MagicString::new(self.code));

            let span = template.span.shrink(1);
            let (pure_url, query) = super::utils::split_url_and_query(span.source_text(self.code));

            let injected_query = inject_query(query, "url");
            let options = rolldown_utils::concat_string!(
              "{ ",
              "eager: true, import: 'default', query: '",
              &injected_query,
              "' }"
            );

            s.update(
              template.span.start as usize,
              template.span.end as usize,
              rolldown_utils::concat_string!(
                "(import.meta.glob('",
                glob,
                "', ",
                options,
                "))['",
                pure_url,
                "']"
              ),
            );
            return;
          }
        }
        _ => return,
      };

      self.urls.push((url, span));
    }
    walk_new_expression(self, it);
  }
}
