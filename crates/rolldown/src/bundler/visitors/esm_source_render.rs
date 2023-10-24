use crate::bundler::module::module::Module;

use super::RendererContext;
use oxc::{
  ast::Visit,
  span::{GetSpan, Span},
};
use rolldown_common::ExportsKind;

pub struct EsmSourceRender<'ast> {
  ctx: RendererContext<'ast>,
}

impl<'ast> EsmSourceRender<'ast> {
  pub fn new(ctx: RendererContext<'ast>) -> Self {
    Self { ctx }
  }

  pub fn apply(&mut self) {
    let module = self.ctx.module;
    if let Some(s) = self.ctx.generate_namespace_variable_declaration() {
      self.ctx.source.prepend(s);
    }
    let program = module.ast.program();
    self.visit_program(program);
  }
}

impl<'ast> Visit<'ast> for EsmSourceRender<'ast> {
  fn visit_binding_identifier(&mut self, ident: &'ast oxc::ast::ast::BindingIdentifier) {
    self.ctx.visit_binding_identifier(ident);
  }

  fn visit_identifier_reference(&mut self, ident: &'ast oxc::ast::ast::IdentifierReference) {
    self.ctx.visit_identifier_reference(ident);
  }

  fn visit_import_declaration(&mut self, decl: &'ast oxc::ast::ast::ImportDeclaration<'ast>) {
    self.ctx.visit_import_declaration(decl);
  }

  fn visit_export_named_declaration(
    &mut self,
    named_decl: &'ast oxc::ast::ast::ExportNamedDeclaration<'ast>,
  ) {
    if let Some(decl) = &named_decl.declaration {
      self.ctx.remove_node(Span::new(named_decl.span.start, decl.span().start));
      self.visit_declaration(decl);
    } else if named_decl.source.is_some() {
      match self.ctx.get_importee_by_span(named_decl.span) {
        Module::Normal(importee) => {
          if importee.exports_kind == ExportsKind::CommonJs {
            self.ctx.overwrite(
              named_decl.span.start,
              named_decl.span.end,
              self.ctx.generate_import_commonjs_module(
                importee,
                &self.ctx.graph.linker_modules[importee.id],
                true,
              ),
            );
            return;
          }
        }
        Module::External(_) => {} // TODO
      }
      self.ctx.remove_node(named_decl.span);
    } else {
      self.ctx.remove_node(named_decl.span);
    }
  }

  fn visit_export_all_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportAllDeclaration<'ast>,
  ) {
    self.ctx.visit_export_all_declaration(decl);
  }

  fn visit_export_default_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportDefaultDeclaration<'ast>,
  ) {
    match &decl.declaration {
      oxc::ast::ast::ExportDefaultDeclarationKind::Expression(exp) => {
        if let Some(name) = self.ctx.default_symbol_name {
          self.ctx.overwrite(decl.span.start, exp.span().start, format!("var {name} = "));
        }
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(decl) => {
        self.ctx.remove_node(Span::new(decl.span.start, decl.span.start));
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(decl) => {
        self.ctx.remove_node(Span::new(decl.span.start, decl.span.start));
      }
      _ => {}
    }
  }

  fn visit_import_expression(&mut self, expr: &oxc::ast::ast::ImportExpression<'ast>) {
    self.ctx.visit_import_expression(expr);
  }

  fn visit_call_expression(&mut self, expr: &'ast oxc::ast::ast::CallExpression<'ast>) {
    self.ctx.visit_call_expression(expr);
    for arg in &expr.arguments {
      self.visit_argument(arg);
    }
    self.visit_expression(&expr.callee);
  }

  fn visit_statement(&mut self, stmt: &'ast oxc::ast::ast::Statement<'ast>) {
    self.ctx.visit_statement(stmt);
    self.visit_statement_match(stmt);
  }
}
