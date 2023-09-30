pub mod scanner;

use oxc::{
  ast::VisitMut,
  span::{Atom, GetSpan, Span},
};
use rolldown_common::{ModuleId, SymbolRef};
use rustc_hash::FxHashMap;

use super::{
  graph::symbols::{get_reference_final_name, get_symbol_final_name, Symbols},
  module::source_mutation::SourceMutation,
};

pub struct FinalizeContext<'ast> {
  pub symbols: &'ast Symbols,
  pub final_names: &'ast FxHashMap<SymbolRef, Atom>,
  pub id: ModuleId,
  pub source_mutations: &'ast mut Vec<SourceMutation>,
}

pub struct Finalizer<'ast> {
  ctx: FinalizeContext<'ast>,
}

impl<'ast> Finalizer<'ast> {
  pub fn new(ctx: FinalizeContext<'ast>) -> Self {
    Self { ctx }
  }

  pub fn remove_node(&mut self, span: Span) {
    self
      .ctx
      .source_mutations
      .push(SourceMutation::Remove(Box::new(span)));
  }

  pub fn remove_symbol(&mut self, span: Span, name: Atom) {
    self
      .ctx
      .source_mutations
      .push(SourceMutation::RenameSymbol(Box::new((span, name))));
  }
}

impl<'ast, 'p> VisitMut<'ast, 'p> for Finalizer<'ast> {
  fn visit_binding_identifier(&mut self, ident: &'p mut oxc::ast::ast::BindingIdentifier) {
    if let Some(name) = get_symbol_final_name(
      self.ctx.id,
      ident.symbol_id.get().unwrap(),
      self.ctx.symbols,
      self.ctx.final_names,
    ) {
      if ident.name != name {
        self.remove_symbol(ident.span, name.clone());
      }
    }
  }

  fn visit_identifier_reference(&mut self, ident: &'p mut oxc::ast::ast::IdentifierReference) {
    if let Some(name) = get_reference_final_name(
      self.ctx.id,
      ident.reference_id.get().unwrap(),
      self.ctx.symbols,
      self.ctx.final_names,
    ) {
      if ident.name != name {
        self.remove_symbol(ident.span, name.clone());
      }
    }
  }

  fn visit_import_declaration(&mut self, decl: &'p mut oxc::ast::ast::ImportDeclaration<'ast>) {
    self.remove_node(decl.span);
  }

  fn visit_export_named_declaration(
    &mut self,
    named_decl: &'p mut oxc::ast::ast::ExportNamedDeclaration<'ast>,
  ) {
    if let Some(decl) = &mut named_decl.declaration {
      self.remove_node(Span::new(named_decl.span.start, decl.span().start));
      self.visit_declaration(decl);
    } else {
      self.remove_node(named_decl.span);
    }
  }

  fn visit_export_all_declaration(
    &mut self,
    decl: &'p mut oxc::ast::ast::ExportAllDeclaration<'ast>,
  ) {
    self.remove_node(decl.span);
  }

  fn visit_export_default_declaration(
    &mut self,
    decl: &'p mut oxc::ast::ast::ExportDefaultDeclaration<'ast>,
  ) {
    match &decl.declaration {
      oxc::ast::ast::ExportDefaultDeclarationKind::Expression(exp) => {
        self
          .ctx
          .source_mutations
          .push(SourceMutation::AddExportDefaultBindingIdentifier(Box::new(
            Span::new(decl.span.start, exp.span().start),
          )));
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(decl) => {
        self.remove_node(Span::new(decl.span.start, decl.span.start));
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(decl) => {
        self.remove_node(Span::new(decl.span.start, decl.span.start));
      }
      _ => {}
    }
  }
}
