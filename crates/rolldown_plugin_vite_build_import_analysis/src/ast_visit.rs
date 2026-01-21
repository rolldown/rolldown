use std::path::PathBuf;

use oxc::{
  allocator::CloneIn as _,
  ast::{
    NONE,
    ast::{self, BindingPattern, Expression, ImportOrExportKind, Statement, VariableDeclaration},
  },
  ast_visit::{VisitMut, walk_mut},
  semantic::ScopeFlags,
  span::SPAN,
};
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_plugin_utils::constants::RemovedPureCSSFilesCache;
use sugar_path::SugarPath;

use super::PRELOAD_HELPER_ID;

const PRELOAD_METHOD: &str = "__vitePreload";

#[expect(clippy::struct_excessive_bools)]
pub struct BuildImportAnalysisVisitor<'a> {
  pub snippet: AstSnippet<'a>,
  pub scope_stack: Vec<ScopeFlags>,
  pub insert_preload: bool,
  pub has_inserted_helper: bool,
  pub need_prepend_helper: bool,
  pub render_built_url: bool,
  pub is_relative_base: bool,
  pub is_modern: bool,
}

impl<'a> VisitMut<'a> for BuildImportAnalysisVisitor<'a> {
  fn visit_program(&mut self, it: &mut oxc::ast::ast::Program<'a>) {
    walk_mut::walk_program(self, it);
    if self.need_prepend_helper && self.insert_preload && !self.has_inserted_helper {
      it.body.push(Statement::from(self.snippet.builder.module_declaration_import_declaration(
        SPAN,
        Some(self.snippet.builder.vec1(
          self.snippet.builder.import_declaration_specifier_import_specifier(
            SPAN,
            self.snippet.builder.module_export_name_identifier_name(SPAN, PRELOAD_METHOD),
            self.snippet.id(PRELOAD_METHOD, SPAN),
            ImportOrExportKind::Value,
          ),
        )),
        self.snippet.builder.string_literal(SPAN, PRELOAD_HELPER_ID, None),
        None,
        NONE,
        ImportOrExportKind::Value,
      )));
    }
  }

  fn visit_expression(&mut self, expr: &mut Expression<'a>) {
    if self.insert_preload {
      let is_rewritten = match expr {
        Expression::CallExpression(expr) => self.rewrite_call_expr(expr),
        Expression::StaticMemberExpression(expr) => self.rewrite_member_expr(expr),
        _ => self.rewrite_import_expr(expr),
      };
      if is_rewritten {
        self.need_prepend_helper = true;
        return;
      }
    }
    walk_mut::walk_expression(self, expr);
  }

  fn visit_import_declaration(&mut self, it: &mut oxc::ast::ast::ImportDeclaration<'a>) {
    it.with_clause.take();
  }

  /// transform `const {foo} = await import('foo')`
  /// to `const {foo} = await __vitePreload(async () => { let foo; return {foo} = await import('foo'); }, ...)`
  fn visit_variable_declaration(&mut self, decl: &mut VariableDeclaration<'a>) {
    if self.insert_preload {
      for decl in &mut decl.declarations {
        if matches!(decl.id, BindingPattern::ObjectPattern(_))
          && matches!(
            &decl.init,
            Some(Expression::AwaitExpression(expr)) if matches!(expr.argument, Expression::ImportExpression(_))
          )
        {
          decl.init = Some(self.snippet.builder.expression_await(
            SPAN,
            self.construct_vite_preload_call(
              decl.id.clone_in(self.snippet.alloc()),
              decl.init.take().unwrap(),
            ),
          ));
          self.need_prepend_helper = true;
        } else {
          walk_mut::walk_variable_declarator(self, decl);
        }
      }
      return;
    }
    walk_mut::walk_variable_declaration(self, decl);
  }

  fn visit_variable_declarator(&mut self, it: &mut oxc::ast::ast::VariableDeclarator<'a>) {
    // Only check if there needs to insert helper function
    if self.insert_preload && self.is_top_level() {
      if let BindingPattern::BindingIdentifier(id) = &it.id {
        self.has_inserted_helper = id.name == PRELOAD_METHOD;
      }
    }
    walk_mut::walk_variable_declarator(self, it);
  }

  fn enter_scope(
    &mut self,
    flags: ScopeFlags,
    _scope_id: &std::cell::Cell<Option<oxc::semantic::ScopeId>>,
  ) {
    self.scope_stack.push(flags);
  }

  fn leave_scope(&mut self) {
    self.scope_stack.pop();
  }
}

pub struct DynamicImportVisitor<'a, 'b> {
  pub chunk_filename_dir: PathBuf,
  pub removed_pure_css_files: &'a RemovedPureCSSFilesCache,
  pub s: &'a mut Option<string_wizard::MagicString<'b>>,
  pub code: &'b str,
}

impl VisitMut<'_> for DynamicImportVisitor<'_, '_> {
  fn visit_import_expression(&mut self, it: &mut ast::ImportExpression<'_>) {
    let value = match &it.source {
      Expression::StringLiteral(s) => Some(s.value),
      Expression::TemplateLiteral(t) => t.single_quasi(),
      _ => None,
    };
    if let Some(url) = value {
      let normalized = self.chunk_filename_dir.join(url.as_str()).normalize();
      if self.removed_pure_css_files.inner.contains_key(normalized.to_slash_lossy().as_ref()) {
        let s = self.s.get_or_insert_with(|| string_wizard::MagicString::new(self.code));
        s.update(
          it.span.start,
          it.span.end,
          format!(
            "Promise.resolve({{{:width$}}})",
            "",
            width = (it.span.end - it.span.start).saturating_sub(19) as usize
          ),
        )
        .expect("update should not fail in build import analysis plugin");
        return;
      }
    }
    walk_mut::walk_import_expression(self, it);
  }
}

pub struct DynamicImport {
  pub start: usize,
  pub end: usize,
  pub source: Option<String>,
}

pub struct DynamicImportCollectVisitor<'a> {
  pub imports: &'a mut Vec<DynamicImport>,
}

impl VisitMut<'_> for DynamicImportCollectVisitor<'_> {
  fn visit_import_expression(&mut self, it: &mut ast::ImportExpression<'_>) {
    let url = match &it.source {
      Expression::StringLiteral(s) => Some(s.value.to_string()),
      Expression::TemplateLiteral(t) => t.single_quasi().map(|s| s.to_string()),
      _ => None,
    };
    self.imports.push(DynamicImport {
      start: it.span.start as usize,
      end: it.span.end as usize,
      source: url,
    });
  }
}
