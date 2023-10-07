pub mod scanner;

use index_vec::IndexVec;
use oxc::{
  ast::Visit,
  span::{Atom, GetSpan, Span},
};
use rolldown_common::{ImportRecord, ImportRecordId, ModuleId, SymbolRef};
use rustc_hash::FxHashMap;
use string_wizard::{MagicString, UpdateOptions};

use super::graph::symbols::{get_reference_final_name, get_symbol_final_name, Symbols};

pub struct RendererContext<'ast> {
  pub symbols: &'ast Symbols,
  pub final_names: &'ast FxHashMap<SymbolRef, Atom>,
  pub id: ModuleId,
  pub default_export_symbol: Option<SymbolRef>,
  pub source: &'ast mut MagicString<'static>,
  pub dynamic_import_request_to_import_record_id: &'ast FxHashMap<Atom, ImportRecordId>,
  pub entries_chunk_final_names: &'ast FxHashMap<ModuleId, String>,
  pub import_records: &'ast IndexVec<ImportRecordId, ImportRecord>,
}

pub struct SourceRenderer<'ast> {
  ctx: RendererContext<'ast>,
}

impl<'ast> SourceRenderer<'ast> {
  pub fn new(ctx: RendererContext<'ast>) -> Self {
    Self { ctx }
  }

  fn overwrite(&mut self, start: u32, end: u32, content: String) {
    self.ctx.source.update_with(
      start,
      end,
      content,
      UpdateOptions {
        overwrite: true,
        ..Default::default()
      },
    );
  }

  pub fn remove_node(&mut self, span: Span) {
    self.ctx.source.remove(span.start, span.end);
  }

  pub fn rename_symbol(&mut self, span: Span, name: Atom) {
    self.overwrite(span.start, span.end, name.to_string());
  }
}

impl<'ast> Visit<'ast> for SourceRenderer<'ast> {
  fn visit_binding_identifier(&mut self, ident: &'ast oxc::ast::ast::BindingIdentifier) {
    if let Some(name) = get_symbol_final_name(
      self.ctx.id,
      ident.symbol_id.get().unwrap(),
      self.ctx.symbols,
      self.ctx.final_names,
    ) {
      if ident.name != name {
        self.rename_symbol(ident.span, name.clone());
      }
    }
  }

  fn visit_identifier_reference(&mut self, ident: &'ast oxc::ast::ast::IdentifierReference) {
    if let Some(name) = get_reference_final_name(
      self.ctx.id,
      ident.reference_id.get().unwrap(),
      self.ctx.symbols,
      self.ctx.final_names,
    ) {
      if ident.name != name {
        self.rename_symbol(ident.span, name.clone());
      }
    }
  }

  fn visit_import_declaration(&mut self, decl: &'ast oxc::ast::ast::ImportDeclaration<'ast>) {
    self.remove_node(decl.span);
  }

  fn visit_export_named_declaration(
    &mut self,
    named_decl: &'ast oxc::ast::ast::ExportNamedDeclaration<'ast>,
  ) {
    if let Some(decl) = &named_decl.declaration {
      self.remove_node(Span::new(named_decl.span.start, decl.span().start));
      self.visit_declaration(decl);
    } else {
      self.remove_node(named_decl.span);
    }
  }

  fn visit_export_all_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportAllDeclaration<'ast>,
  ) {
    self.remove_node(decl.span);
  }

  fn visit_export_default_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportDefaultDeclaration<'ast>,
  ) {
    match &decl.declaration {
      oxc::ast::ast::ExportDefaultDeclarationKind::Expression(exp) => {
        let canonical_ref = self
          .ctx
          .symbols
          .par_get_canonical_ref(self.ctx.default_export_symbol.unwrap());
        let canonical_name = self.ctx.final_names.get(&canonical_ref).unwrap().clone();
        self.overwrite(
          decl.span.start,
          exp.span().start,
          format!("var {canonical_name} = "),
        );
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

  fn visit_import_expression(&mut self, expr: &'p mut oxc::ast::ast::ImportExpression<'ast>) {
    if let oxc::ast::ast::Expression::StringLiteral(str) = &mut expr.source {
      if let Some(import_record_id) = self
        .ctx
        .dynamic_import_request_to_import_record_id
        .get(&str.value)
      {
        let module_id = self.ctx.import_records[*import_record_id].resolved_module;
        if let Some(name) = self.ctx.entries_chunk_final_names.get(&module_id) {
          self.ctx.source_mutations.push(Box::new(Overwrite {
            span: Span::new(str.span.start, str.span.end),
            content: format!("'./{}'", name.clone()),
          }));
        }
      }
    }
  }
}
