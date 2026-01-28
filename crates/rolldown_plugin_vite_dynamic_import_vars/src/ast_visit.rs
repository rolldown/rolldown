use std::{borrow::Cow, path::Path};

use cow_utils::CowUtils;
use oxc::{
  ast::{Comment, ast::Expression},
  ast_visit::{Visit, walk},
};
use rolldown_plugin::{LogWithoutPlugin, PluginContext};
use string_wizard::MagicString;
use sugar_path::SugarPath as _;

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

impl<'ast> Visit<'ast> for DynamicImportVarsVisit<'ast, '_> {
  fn visit_expression(&mut self, expr: &Expression<'ast>) {
    if self.rewrite_variable_dynamic_import(expr, None) {
      walk::walk_expression(self, expr);
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

      let raw = source.span.shrink(1).source_text(self.source_text);
      let raw_pattern = if &glob[..index] == source.quasis[0].value.raw {
        Cow::Borrowed(raw)
      } else {
        let mut s = String::with_capacity(index + source.quasis[0].value.raw.len());
        s.push_str(&glob[..index]);
        s.push_str(&raw[source.quasis[0].value.raw.len()..]);
        Cow::Owned(s)
      };

      let base = self.importer.parent().unwrap_or(self.root);
      let normalized = if raw_pattern.as_bytes()[0] == b'/' {
        self.root.join(&raw_pattern[1..]).relative(base)
      } else {
        base.join(raw_pattern.as_ref()).relative(base)
      };

      let normalized = normalized.to_slash_lossy();
      let new_raw_pattern = if normalized.starts_with("./") || normalized.starts_with("../") {
        normalized.into_owned()
      } else {
        rolldown_utils::concat_string!("./", normalized)
      };

      let glob = glob.cow_replace("**", "*");
      let source_text = source.span.source_text(self.source_text);

      let (pattern, glob_params) = {
        let index = glob.rfind('/').unwrap_or(0);
        let index = glob[index..].find('?').map_or(glob.len(), |i| i + index);

        let (glob, query) = glob.split_at(index);
        let glob = match to_valid_glob(glob, source_text) {
          Ok(glob) => glob,
          Err(error) => {
            self.ctx.warn(LogWithoutPlugin { message: error.to_string(), ..Default::default() });
            return false;
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

      self
        .magic_string
        .get_or_insert_with(|| MagicString::new(self.source_text))
        .update(import_expr.span.start, import_expr.span.end, replacement)
        .expect("update should not fail in dynamic import vars plugin");

      self.need_helper = true;
      return false;
    }
    true
  }
}
