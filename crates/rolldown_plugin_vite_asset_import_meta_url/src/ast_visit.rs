use std::ops::Range;

use cow_utils::CowUtils as _;
use oxc::{
  ast::{Comment, ast::Argument},
  ast_visit::{VisitMut, walk_mut::walk_new_expression},
};
use rolldown_ecmascript_utils::ExpressionExt;
use rolldown_plugin::{LogWithoutPlugin, PluginContext};
use rolldown_plugin_utils::inject_query;

pub struct NewUrlVisitor<'a, 'b, 'ast> {
  pub urls: &'a mut Vec<(String, Range<u32>, &'b str)>,
  pub s: &'a mut Option<string_wizard::MagicString<'b>>,
  pub code: &'b str,
  pub ctx: &'a PluginContext,
  pub current_comment: usize,
  pub comments: oxc::allocator::Vec<'ast, Comment>,
}

impl NewUrlVisitor<'_, '_, '_> {
  /// Respects @vite-ignore comment (e.g., import(/* @vite-ignore */ `..`))
  fn is_vite_ignore_comment(&mut self, span: oxc::span::Span) -> bool {
    if self.current_comment < self.comments.len() {
      for comment in &self.comments[self.current_comment..] {
        if comment.attached_to > span.start {
          break;
        }
        self.current_comment += 1;
        if comment.attached_to == span.start && comment.is_vite() {
          return true;
        }
      }
    }
    false
  }
}

impl<'ast> VisitMut<'ast> for NewUrlVisitor<'_, '_, 'ast> {
  fn visit_new_expression(&mut self, it: &mut oxc::ast::ast::NewExpression<'ast>) {
    if it.callee.is_specific_id("URL") && it.arguments.len() == 2 {
      if !it.arguments[1].as_expression().is_some_and(ExpressionExt::is_import_meta_url) {
        return;
      }

      let (url, span) = match &it.arguments[0] {
        Argument::StringLiteral(lit) if self.is_vite_ignore_comment(lit.span) => return,
        Argument::TemplateLiteral(template) if self.is_vite_ignore_comment(template.span) => return,
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

            let glob = glob.cow_replace("**", "*");
            let (glob, query) = {
              let index = glob.rfind('/').unwrap_or(0);
              let index = glob[index..].find('?').map_or(glob.len(), |i| i + index);
              glob.split_at(index)
            };

            let span = template.span.shrink(1);
            let pure_url = super::utils::strip_query(span.source_text(self.code));

            let injected_query = inject_query(query, "url");
            let options = rolldown_utils::concat_string!(
              "{ ",
              "eager: true, import: 'default', query: '",
              &injected_query,
              "' }"
            );

            s.update(
              template.span.start,
              template.span.end,
              rolldown_utils::concat_string!(
                "(import.meta.glob('",
                glob,
                "', ",
                options,
                "))[`",
                pure_url,
                "`]"
              ),
            )
            .expect("update should not fail in asset import meta url plugin");
            return;
          }
        }
        _ => return,
      };

      let span = span.shrink(1);
      self.urls.push((url, span.start..span.end, it.span.source_text(self.code)));
    }
    walk_new_expression(self, it);
  }
}
