use std::path::Path;

use cow_utils::CowUtils;
use oxc::{
  ast::ast::{Expression, Program},
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

/// A variable dynamic import whose glob needs the resolver, copied out of the AST.
///
/// It is rewritten after the AST is dropped, because the AST is not `Send` and can't be held
/// across the `.await` on the resolver. Blocking on the resolver instead can deadlock: a blocked
/// worker hands its core to a thread from the small blocking pool, and once enough transforms
/// block at once, no core is left to run the resolver.
pub struct AsyncImport {
  pub glob: String,
  pub span: Span,
  pub source_span: Span,
  pub first_quasi_len: usize,
}

pub struct DynamicImportVarsVisit<'a> {
  pub ctx: &'a PluginContext,
  pub source_text: &'a str,
  pub root: &'a Path,
  pub importer: &'a Path,
  pub need_helper: bool,
  pub vite_ignored_starts: Vec<u32>,
  pub async_imports: Vec<AsyncImport>,
  pub magic_string: Option<MagicString<'a>>,
}

impl<'ast> VisitJs<'ast> for DynamicImportVarsVisit<'_> {
  fn visit_program(&mut self, program: &Program<'ast>) {
    self.vite_ignored_starts = program
      .comments
      .iter()
      .filter(|comment| comment.is_vite())
      .map(|comment| comment.attached_to)
      .collect();
    walk_js::walk_program(self, program);
  }

  fn visit_expression(&mut self, expr: &Expression<'ast>) {
    if self.rewrite_variable_dynamic_import(expr) {
      walk_js::walk_expression(self, expr);
    }
  }
}

impl DynamicImportVarsVisit<'_> {
  fn rewrite_variable_dynamic_import(&mut self, expr: &Expression<'_>) -> bool {
    if let Expression::ImportExpression(import_expr) = expr
      && let Expression::TemplateLiteral(source) = &import_expr.source
    {
      // Respects @vite-ignore comment (e.g., import(/* @vite-ignore */ `..`))
      if self.vite_ignored_starts.contains(&source.span.start) {
        return false;
      }

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

      // Oxc turns CRLF into LF in `raw`, so take the length from the span.
      let first_quasi_len = source.quasis[0].span.size() as usize;
      if glob.as_bytes()[0] != b'.' && glob.as_bytes()[0] != b'/' {
        self.async_imports.push(AsyncImport {
          glob: glob.into_owned(),
          span: import_expr.span,
          source_span: source.span,
          first_quasi_len,
        });
        return false;
      }

      let prefix = to_usv_str(source.quasis[0].value.cooked.expect("untagged template literal"));
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
