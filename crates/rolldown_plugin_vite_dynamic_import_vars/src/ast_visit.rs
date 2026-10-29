use std::{borrow::Cow, path::Path};

use cow_utils::CowUtils;
use oxc::{
  ast::{Comment, ast::Expression},
  ast_visit::{VisitJs, walk_js},
  span::Span,
};
use rolldown_plugin::{LogWithoutPlugin, PluginContext};
use rolldown_plugin_utils::to_string_literal;
use rolldown_std_utils::relative_path_as_js_specifier;
use rolldown_utils::ecmascript::{escape_string_for_template_literal, to_usv_str};
use string_wizard::MagicString;

use super::dynamic_import_to_glob::{
  has_special_query_param, should_ignore, template_literal_to_glob, to_valid_glob,
};

#[derive(Debug)]
struct DynamicImportRequest<'a> {
  pub query: &'a str,
  pub import: bool,
}

pub struct DynamicImportVarsVisit<'ast, 'b> {
  pub ctx: &'b PluginContext,
  pub source_text: &'b str,
  pub root: &'b Path,
  pub importer: &'b Path,
  pub need_helper: bool,
  pub comments: &'b oxc::allocator::Vec<'ast, Comment>,
  pub current_comment: usize,
  pub async_imports: Vec<String>,
  pub async_imports_addrs: Vec<*const Expression<'ast>>,
  pub magic_string: Option<MagicString<'b>>,
}

impl<'ast> VisitJs<'ast> for DynamicImportVarsVisit<'ast, '_> {
  fn visit_expression(&mut self, expr: &Expression<'ast>) {
    if self.rewrite_variable_dynamic_import(expr, None) {
      walk_js::walk_expression(self, expr);
    }
  }
}

impl<'ast> DynamicImportVarsVisit<'ast, '_> {
  pub fn rewrite_variable_dynamic_import(
    &mut self,
    expr: &Expression<'ast>,
    async_imports: Option<&str>,
  ) -> bool {
    if let Expression::ImportExpression(import_expr) = expr
      && let Expression::TemplateLiteral(source) = &import_expr.source
    {
      // Respects @vite-ignore comment (e.g., import(/* @vite-ignore */ `..`))
      if self.current_comment < self.comments.len() {
        for comment in &self.comments[self.current_comment..] {
          if comment.attached_to > source.span.start {
            break;
          }
          self.current_comment += 1;
          if comment.attached_to == source.span.start && comment.is_vite() {
            return false;
          }
        }
      }
      let glob = match async_imports {
        Some(glob) => Cow::Borrowed(glob),
        None => {
          if source.is_no_substitution_template() {
            return false;
          }

          let glob = match template_literal_to_glob(source) {
            Ok(glob) => glob,
            Err(error) => {
              self.ctx.warn(LogWithoutPlugin { message: error.to_string(), ..Default::default() });
              return false;
            }
          };

          if memchr::memchr(b'*', glob.as_bytes()).is_none() || should_ignore(&glob) {
            return false;
          }

          if glob.as_bytes()[0] != b'.' && glob.as_bytes()[0] != b'/' {
            self.async_imports.push(glob.into_owned());
            self.async_imports_addrs.push(std::ptr::from_ref(expr));
            return false;
          }

          glob
        }
      };

      let Some(index) = memchr::memchr(b'*', glob.as_bytes()) else {
        return false;
      };

      // Oxc turns CRLF into LF in `raw`, so take the length from the span.
      let first_quasi_len = source.quasis[0].span.size() as usize;
      let prefix = if async_imports.is_some() {
        Cow::Borrowed(&glob[..index])
      } else {
        to_usv_str(source.quasis[0].value.cooked.expect("untagged template literal"))
      };
      self.rewrite(import_expr.span, source.span, first_quasi_len, &prefix, &glob);
      return false;
    }
    true
  }

  /// Rewrites the `import(...)` at `span` into a glob import. `source_span` is the span of its
  /// template literal, `first_quasi_len` is the source byte length of its first quasi, and
  /// `prefix` is the cooked or resolved static runtime path prefix.
  pub fn rewrite(
    &mut self,
    span: Span,
    source_span: Span,
    first_quasi_len: usize,
    prefix: &str,
    glob: &str,
  ) {
    let raw = source_span.shrink(1).source_text(self.source_text);
    let base = self.importer.parent().unwrap_or(self.root);
    let runtime_prefix = if prefix.is_empty() {
      String::new()
    } else {
      // The final segment can be a partial filename, such as `..` in `..${name}.js`.
      // Append a filename character so normalization preserves it, then remove that character.
      let prefix_with_suffix = format!("{prefix}_");
      let mut normalized = if let Some(prefix) = prefix_with_suffix.strip_prefix('/') {
        relative_path_as_js_specifier(self.root.join(prefix), base)
      } else {
        relative_path_as_js_specifier(base.join(&prefix_with_suffix), base)
      };
      normalized.truncate(normalized.len() - 1);
      normalized
    };
    // Escape the static prefix for a template literal and keep the original expressions as source.
    let runtime_prefix = escape_string_for_template_literal(runtime_prefix.as_str().into());
    let runtime_pattern = format!("`{runtime_prefix}{}`", &raw[first_quasi_len..]);

    let glob = glob.cow_replace("**", "*");
    let source_text = source_span.source_text(self.source_text);

    let (pattern, glob_params) = {
      let index = glob.rfind('/').unwrap_or(0);
      let index = glob[index..].find('?').map_or(glob.len(), |i| i + index);

      let (glob, query) = glob.split_at(index);
      let glob = match to_valid_glob(glob, source_text) {
        Ok(glob) => glob,
        Err(error) => {
          self.ctx.warn(LogWithoutPlugin { message: error.to_string(), ..Default::default() });
          return;
        }
      };

      let params = (!query.is_empty())
        .then_some(DynamicImportRequest { query, import: has_special_query_param(query) });

      (glob, params)
    };

    // __variableDynamicImportRuntimeHelper((import.meta.glob(pattern, params)), expr, segments)
    let segments = pattern.split('/').count();
    let replacement = format!(
      "__variableDynamicImportRuntimeHelper(import.meta.glob({}{}), {runtime_pattern}, {segments})",
      to_string_literal(&pattern),
      glob_params
        .map(|params| {
          format!(
            ", {{ query: {}{} }}",
            to_string_literal(params.query),
            if params.import { ", import: \"*\"" } else { "" }
          )
        })
        .unwrap_or_default()
    );

    self
      .magic_string
      .get_or_insert_with(|| MagicString::new(self.source_text))
      .update(span.start, span.end, replacement)
      .expect("update should not fail in dynamic import vars plugin");

    self.need_helper = true;
  }
}
