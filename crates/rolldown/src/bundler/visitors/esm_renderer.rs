use crate::bundler::module::Module;

use super::RendererBase;
use oxc::{
  ast::Visit,
  span::{GetSpan, Span},
};
use rolldown_common::ExportsKind;

pub struct EsmRenderer<'ast> {
  base: RendererBase<'ast>,
}

impl<'ast> EsmRenderer<'ast> {
  pub fn new(base: RendererBase<'ast>) -> Self {
    Self { base }
  }

  pub fn apply(&mut self) {
    let module = self.base.module;
    if let Some(s) = self.base.generate_namespace_variable_declaration() {
      self.base.source.prepend(s);
    }
    let program = module.ast.program();
    self.visit_program(program);
  }
}

impl<'ast> Visit<'ast> for EsmRenderer<'ast> {
  fn visit_binding_identifier(&mut self, ident: &'ast oxc::ast::ast::BindingIdentifier) {
    self.base.visit_binding_identifier(ident);
  }

  fn visit_identifier_reference(&mut self, ident: &'ast oxc::ast::ast::IdentifierReference) {
    self.base.visit_identifier_reference(ident, false);
  }

  fn visit_import_declaration(&mut self, decl: &'ast oxc::ast::ast::ImportDeclaration<'ast>) {
    self.base.visit_import_declaration(decl);
  }

  fn visit_export_named_declaration(
    &mut self,
    named_decl: &'ast oxc::ast::ast::ExportNamedDeclaration<'ast>,
  ) {
    if let Some(decl) = &named_decl.declaration {
      self.base.remove_node(Span::new(named_decl.span.start, decl.span().start));
      self.visit_declaration(decl);
    } else if named_decl.source.is_some() {
      match self.base.get_importee_by_span(named_decl.span) {
        Module::Normal(importee) => {
          if importee.exports_kind == ExportsKind::CommonJs {
            self.base.overwrite(
              named_decl.span.start,
              named_decl.span.end,
              self.base.generate_import_commonjs_module(
                importee,
                &self.base.graph.linking_infos[importee.id],
                true,
              ),
            );
            return;
          }
        }
        Module::External(_) => {} // TODO
      }
      self.base.remove_node(named_decl.span);
    } else {
      self.base.remove_node(named_decl.span);
    }
  }

  fn visit_export_all_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportAllDeclaration<'ast>,
  ) {
    self.base.visit_export_all_declaration(decl);
  }

  fn visit_export_default_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportDefaultDeclaration<'ast>,
  ) {
    match &decl.declaration {
      oxc::ast::ast::ExportDefaultDeclarationKind::Expression(exp) => {
        if let Some(name) = self.base.default_symbol_name {
          self.base.overwrite(decl.span.start, exp.span().start, format!("var {name} = "));
        }
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(decl) => {
        self.base.remove_node(Span::new(decl.span.start, decl.span.start));
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(decl) => {
        self.base.remove_node(Span::new(decl.span.start, decl.span.start));
      }
      _ => {}
    }
  }

  fn visit_import_expression(&mut self, expr: &oxc::ast::ast::ImportExpression<'ast>) {
    self.base.visit_import_expression(expr);
  }

  fn visit_call_expression(&mut self, expr: &'ast oxc::ast::ast::CallExpression<'ast>) {
    self.base.visit_call_expression(expr);
    for arg in &expr.arguments {
      self.visit_argument(arg);
    }
    if let oxc::ast::ast::Expression::Identifier(s) = &expr.callee {
      self.base.visit_identifier_reference(s, true);
    } else {
      self.visit_expression(&expr.callee);
    }
  }

  fn visit_statement(&mut self, stmt: &'ast oxc::ast::ast::Statement<'ast>) {
    self.base.visit_statement(stmt);
    self.visit_statement_match(stmt);
  }
}
