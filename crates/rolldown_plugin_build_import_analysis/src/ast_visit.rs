use oxc::{
  allocator::CloneIn as _,
  ast::{
    NONE,
    ast::{BindingPatternKind, Expression, ImportOrExportKind, Statement, VariableDeclaration},
  },
  ast_visit::{VisitMut, walk_mut},
  semantic::ScopeFlags,
  span::SPAN,
};
use rolldown_ecmascript_utils::AstSnippet;

use super::PRELOAD_HELPER_ID;

const PRELOAD_METHOD: &str = "__vitePreload";

#[allow(clippy::struct_excessive_bools)]
pub struct BuildImportAnalysisVisitor<'a> {
  pub snippet: AstSnippet<'a>,
  pub scope_stack: Vec<ScopeFlags>,
  pub insert_preload: bool,
  pub has_inserted_helper: bool,
  pub need_prepend_helper: bool,
  pub render_built_url: bool,
  pub is_relative_base: bool,
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
    walk_mut::walk_expression(self, expr);
    if self.insert_preload {
      match expr {
        Expression::CallExpression(expr) => self.rewrite_import_expr(expr),
        Expression::StaticMemberExpression(expr) => self.rewrite_member_expr(expr),
        _ => return,
      }
      self.need_prepend_helper = true;
    }
  }

  fn visit_import_declaration(&mut self, it: &mut oxc::ast::ast::ImportDeclaration<'a>) {
    it.with_clause.take();
  }

  fn visit_variable_declarator(&mut self, it: &mut oxc::ast::ast::VariableDeclarator<'a>) {
    // Only check if there needs to insert helper function
    if self.insert_preload && self.is_top_level() {
      if let BindingPatternKind::BindingIdentifier(id) = &it.id.kind {
        self.has_inserted_helper = id.name == PRELOAD_METHOD;
      }
    }
    walk_mut::walk_variable_declarator(self, it);
  }

  /// transform `const {foo} = await import('foo')`
  /// to `const {foo} = await __vitePreload(async () => { let foo; return {foo} = await import('foo'); }, ...)`
  fn visit_variable_declaration(&mut self, decl: &mut VariableDeclaration<'a>) {
    walk_mut::walk_variable_declaration(self, decl);
    if self.insert_preload {
      for decl in &mut decl.declarations {
        if matches!(decl.id.kind, BindingPatternKind::ObjectPattern(_))
          && matches!(
            &decl.init,
            Some(Expression::AwaitExpression(expr)) if matches!(expr.argument, Expression::ImportExpression(_))
          )
        {
          decl.init = Some(self.snippet.builder.expression_await(
            SPAN,
            self.construct_vite_preload_call(
              self.snippet.builder.binding_pattern(
                decl.id.kind.clone_in(self.snippet.alloc()),
                NONE,
                false,
              ),
              decl.init.take().unwrap(),
            ),
          ));
          self.need_prepend_helper = true;
        }
      }
    }
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
