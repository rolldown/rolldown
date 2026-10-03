use std::{borrow::Cow, path::Path};

use cow_utils::CowUtils;
use oxc::{
  ast::{Comment, ast::Expression},
  ast_visit::{VisitJs, walk_js},
  span::Span,
};
use rolldown_plugin::{LogWithoutPlugin, PluginContext};
use rolldown_std_utils::relative_path_to_slash;

use super::dynamic_import_to_glob::{
  has_special_query_param, should_ignore, template_literal_to_glob, to_valid_glob,
};

#[derive(Debug)]
struct DynamicImportRequest<'a> {
  pub query: &'a str,
  pub import: bool,
}

/// One edit to the source text. `transform` applies it with
/// `magic_string.update(start, end, replacement)`. This struct holds plain data
/// only.
pub struct Edit {
  pub start: u32,
  pub end: u32,
  pub replacement: String,
}

/// This struct describes one glob with a bare specifier or an alias specifier,
/// such as ``import(`$lib/data/${name}.js`)``. The JS resolver must find the
/// file that the specifier points to. This crate builds the replacement only
/// after the resolver answers.
///
/// This struct holds owned data only. `transform` can therefore drop the AST
/// before it calls the resolver. The earlier code kept a `*const Expression`
/// that pointed into the arena.
///
/// The hook cannot await while the AST is alive. The AST is `!Send`, and the
/// hook future must be `Send`. Do not block the thread to avoid this limit.
/// A blocked thread deadlocks the runtime (#10664).
pub struct PendingAsyncImport {
  pub glob: String,
  pub import_span: Span,
  pub source_span: Span,
  pub first_quasi_raw: String,
}

/// This struct holds the data that `build_edit` needs from outside the AST.
#[derive(Clone, Copy)]
pub struct RewriteContext<'b> {
  pub ctx: &'b PluginContext,
  pub source_text: &'b str,
  pub root: &'b Path,
  pub importer: &'b Path,
}

impl RewriteContext<'_> {
  /// Builds the `__variableDynamicImportRuntimeHelper(...)` expression that
  /// replaces one variable dynamic import. This function takes plain data only.
  /// The caller can therefore call it after `transform` drops the AST.
  pub fn build_edit(
    &self,
    glob: &str,
    import_span: Span,
    source_span: Span,
    first_quasi_raw: &str,
  ) -> Option<Edit> {
    let index = memchr::memchr(b'*', glob.as_bytes())?;

    let raw = source_span.shrink(1).source_text(self.source_text);
    let raw_pattern = if &glob[..index] == first_quasi_raw {
      Cow::Borrowed(raw)
    } else {
      let mut s = String::with_capacity(index + first_quasi_raw.len());
      s.push_str(&glob[..index]);
      s.push_str(&raw[first_quasi_raw.len()..]);
      Cow::Owned(s)
    };

    let base = self.importer.parent().unwrap_or(self.root);
    let normalized = if raw_pattern.as_bytes()[0] == b'/' {
      relative_path_to_slash(self.root.join(&raw_pattern[1..]), base)
    } else {
      relative_path_to_slash(base.join(raw_pattern.as_ref()), base)
    };
    let new_raw_pattern = if normalized.starts_with("./") || normalized.starts_with("../") {
      normalized
    } else {
      rolldown_utils::concat_string!("./", normalized)
    };

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
          return None;
        }
      };

      let params = (!query.is_empty())
        .then_some(DynamicImportRequest { query, import: has_special_query_param(query) });

      (glob, params)
    };

    // __variableDynamicImportRuntimeHelper((import.meta.glob(pattern, params)), expr, segments)
    let segments = pattern.split('/').count();
    let replacement = format!(
      "__variableDynamicImportRuntimeHelper(import.meta.glob(\"{pattern}\"{}), `{new_raw_pattern}`, {segments})",
      glob_params
        .map(|params| {
          format!(
            ", {{ query: \"{}\"{} }}",
            params.query,
            if params.import { ", import: \"*\"" } else { "" }
          )
        })
        .unwrap_or_default()
    );

    Some(Edit { start: import_span.start, end: import_span.end, replacement })
  }
}

pub struct DynamicImportVarsVisit<'ast, 'b> {
  pub rewrite: RewriteContext<'b>,
  pub comments: &'b oxc::allocator::Vec<'ast, Comment>,
  pub current_comment: usize,
  pub async_imports: Vec<PendingAsyncImport>,
  pub edits: Vec<Edit>,
}

impl<'ast> VisitJs<'ast> for DynamicImportVarsVisit<'ast, '_> {
  fn visit_expression(&mut self, expr: &Expression<'ast>) {
    if self.rewrite_variable_dynamic_import(expr) {
      walk_js::walk_expression(self, expr);
    }
  }
}

impl<'ast> DynamicImportVarsVisit<'ast, '_> {
  fn rewrite_variable_dynamic_import(&mut self, expr: &Expression<'ast>) -> bool {
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

      if source.is_no_substitution_template() {
        return false;
      }

      let glob = match template_literal_to_glob(source) {
        Ok(glob) => glob,
        Err(error) => {
          self
            .rewrite
            .ctx
            .warn(LogWithoutPlugin { message: error.to_string(), ..Default::default() });
          return false;
        }
      };

      if memchr::memchr(b'*', glob.as_bytes()).is_none() || should_ignore(&glob) {
        return false;
      }

      let first_quasi_raw = source.quasis[0].value.raw.as_str();
      if glob.as_bytes()[0] != b'.' && glob.as_bytes()[0] != b'/' {
        self.async_imports.push(PendingAsyncImport {
          glob: glob.into_owned(),
          import_span: import_expr.span,
          source_span: source.span,
          first_quasi_raw: first_quasi_raw.to_owned(),
        });
        return false;
      }

      if let Some(edit) =
        self.rewrite.build_edit(&glob, import_expr.span, source.span, first_quasi_raw)
      {
        self.edits.push(edit);
      }
      return false;
    }
    true
  }
}
