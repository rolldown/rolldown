use std::{fmt, path::Path};

use arcstr::ArcStr;
use cow_utils::CowUtils;
use oxc::{
  ast::ast::{Expression, ObjectPropertyKind, Program},
  ast_visit::{VisitJs, walk_js},
  span::{GetSpan, Span},
};
use rolldown_error::{BuildDiagnostic, EventKindSwitcher};
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
  pub query: Option<&'a str>,
  pub import: bool,
  pub attributes: Option<&'a str>,
}

fn format_glob_options<'a, 'b: 'a>(
  request: &'b DynamicImportRequest<'a>,
) -> impl fmt::Display + 'a {
  fmt::from_fn(move |formatter| {
    if request.query.is_none() && request.attributes.is_none() {
      return Ok(());
    }
    formatter.write_str(", { ")?;
    if let Some(query) = request.query {
      write!(formatter, "query: {}", to_string_literal(query))?;
      if request.import {
        formatter.write_str(", import: \"*\"")?;
      }
    }
    if let Some(attributes) = request.attributes {
      if request.query.is_some() {
        formatter.write_str(", ")?;
      }
      write!(formatter, "with: {attributes}")?;
    }
    formatter.write_str(" }")
  })
}

#[derive(Default)]
pub struct ImportOptions {
  attributes_span: Option<Span>,
  assert_span: Option<Span>,
}

fn import_options(options: Option<&Expression<'_>>) -> ImportOptions {
  let Some(Expression::ObjectExpression(options)) = options.map(Expression::get_inner_expression)
  else {
    return ImportOptions::default();
  };
  let find_property = |name: &str| {
    options
      .properties
      .iter()
      .rev()
      .filter_map(ObjectPropertyKind::as_property)
      .find(|property| property.key.is_specific_static_name(name))
  };
  let with = find_property("with");
  let assertion = if with.is_none() { find_property("assert") } else { None };
  let attributes = with.or(assertion);
  let attributes_span = match attributes.map(|property| property.value.get_inner_expression()) {
    Some(Expression::ObjectExpression(attributes)) => Some(attributes.span),
    _ => None,
  };
  ImportOptions { attributes_span, assert_span: assertion.map(|property| property.key.span()) }
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
  pub options: ImportOptions,
  pub first_quasi_len: usize,
}

pub struct DynamicImportVarsVisit<'a> {
  pub ctx: &'a PluginContext,
  pub source_text: &'a ArcStr,
  pub root: &'a Path,
  pub id: &'a str,
  pub need_helper: bool,
  pub vite_ignored_starts: Vec<u32>,
  pub async_imports: Vec<AsyncImport>,
  pub magic_string: Option<MagicString<'a>>,
  pub warnings: Vec<BuildDiagnostic>,
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
      let options = import_options(import_expr.options.as_ref());
      if glob.as_bytes()[0] != b'.' && glob.as_bytes()[0] != b'/' {
        self.async_imports.push(AsyncImport {
          glob: glob.into_owned(),
          span: import_expr.span,
          source_span: source.span,
          options,
          first_quasi_len,
        });
        return false;
      }

      let prefix = to_usv_str(source.quasis[0].value.cooked.expect("untagged template literal"));
      self.rewrite(import_expr.span, source.span, &options, first_quasi_len, &prefix, &glob);
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
    options: &ImportOptions,
    first_quasi_len: usize,
    prefix: &str,
    glob: &str,
  ) {
    let raw = source_span.shrink(1).source_text(self.source_text);
    let base = Path::new(self.id).parent().unwrap_or(self.root);
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

    let (pattern, request) = {
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

      let query = (!query.is_empty()).then_some(query);
      (
        glob,
        DynamicImportRequest {
          query,
          import: query.is_some_and(has_special_query_param),
          attributes: options.attributes_span.map(|span| span.source_text(self.source_text)),
        },
      )
    };

    if let Some(span) = options.assert_span
      && self.ctx.options().checks.contains(EventKindSwitcher::DeprecatedImportAssertion)
    {
      self.warnings.push(
        BuildDiagnostic::deprecated_import_assertion(
          self.id.to_string(),
          self.source_text.clone(),
          span,
        )
        .with_severity_warning(),
      );
    }

    // __variableDynamicImportRuntimeHelper((import.meta.glob(pattern, params)), expr, segments)
    let segments = pattern.split('/').count();
    let replacement = format!(
      "__variableDynamicImportRuntimeHelper(import.meta.glob({}{}), {runtime_pattern}, {segments})",
      to_string_literal(&pattern),
      format_glob_options(&request),
    );

    self
      .magic_string
      .get_or_insert_with(|| MagicString::new(self.source_text))
      .update(span.start, span.end, replacement)
      .expect("update should not fail in dynamic import vars plugin");

    self.need_helper = true;
  }
}
