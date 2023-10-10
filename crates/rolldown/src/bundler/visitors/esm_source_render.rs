use oxc::{
  ast::Visit,
  span::{Atom, GetSpan, Span},
};
use rolldown_common::ModuleResolution;

use crate::bundler::module::module::Module;

use super::RendererContext;

pub struct EsmSourceRender<'ast> {
  ctx: RendererContext<'ast>,
}

impl<'ast> EsmSourceRender<'ast> {
  pub fn new(ctx: RendererContext<'ast>) -> Self {
    Self { ctx }
  }

  pub fn apply(&mut self) {
    let module = self.ctx.module;
    let program = module.ast.program();
    self.visit_program(program);

    if let Some(namespace_name) = self.ctx.namespace_symbol_name {
      let exports: String = module
        .resolved_exports
        .iter()
        .map(|(exported_name, info)| {
          let canonical_ref = self.ctx.symbols.par_get_canonical_ref(info.local_symbol);
          let canonical_name = self.ctx.final_names.get(&canonical_ref).unwrap();
          format!("  get {exported_name}() {{ return {canonical_name} }}",)
        })
        .collect::<Vec<_>>()
        .join(",\n");
      self
        .ctx
        .source
        .append(format!("\nvar {namespace_name} = {{\n{exports}\n}};\n",));
    }
  }
}

impl<'ast> Visit<'ast> for EsmSourceRender<'ast> {
  fn visit_binding_identifier(&mut self, ident: &'ast oxc::ast::ast::BindingIdentifier) {
    if let Some(name) = self
      .ctx
      .get_symbol_final_name(self.ctx.module.id, ident.symbol_id.get().unwrap())
    {
      if ident.name != name {
        self.ctx.rename_symbol(ident.span, name.clone());
      }
    }
  }

  fn visit_identifier_reference(&mut self, ident: &'ast oxc::ast::ast::IdentifierReference) {
    if let Some(name) = self
      .ctx
      .get_reference_final_name(self.ctx.module.id, ident.reference_id.get().unwrap())
    {
      if ident.name != name {
        self.ctx.rename_symbol(ident.span, name.clone());
      }
    }
  }

  fn visit_import_declaration(&mut self, decl: &'ast oxc::ast::ast::ImportDeclaration<'ast>) {
    self.ctx.remove_node(decl.span);
    let rec =
      &self.ctx.module.import_records[self.ctx.module.imports.get(&decl.span).copied().unwrap()];
    let importee = &self.ctx.modules[rec.resolved_module];
    if let Module::Normal(importee) = importee {
      if importee.module_resolution == ModuleResolution::CommonJs {
        // add import cjs symbol binding
        let namespace_name = self
          .ctx
          .get_symbol_final_name(importee.id, importee.namespace_symbol.0.symbol)
          .unwrap();
        let wrap_symbol_name = self
          .ctx
          .get_symbol_final_name(importee.id, importee.wrap_symbol.unwrap())
          .unwrap();
        self.ctx.source.prepend_left(
          decl.span.start,
          format!("var {namespace_name} = __toESM({wrap_symbol_name}());\n"),
        );
        decl.specifiers.iter().for_each(|s| match s {
          oxc::ast::ast::ImportDeclarationSpecifier::ImportSpecifier(spec) => {
            if let Some(name) = self.ctx.get_symbol_final_name(
              importee.id,
              importee
                .cjs_symbols
                .get(spec.imported.name())
                .unwrap()
                .symbol,
            ) {
              self.ctx.source.prepend_left(
                decl.span.start,
                format!("var {name} = {namespace_name}.{name};\n"),
              );
            }
          }
          oxc::ast::ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => {
            if let Some(name) = self.ctx.get_symbol_final_name(
              importee.id,
              importee
                .cjs_symbols
                .get(&Atom::new_inline("default"))
                .unwrap()
                .symbol,
            ) {
              self.ctx.source.prepend_left(
                decl.span.start,
                format!("var {name} = {namespace_name}.default;\n"),
              );
            }
          }
          oxc::ast::ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {}
        });
      }
    }
  }

  fn visit_export_named_declaration(
    &mut self,
    named_decl: &'ast oxc::ast::ast::ExportNamedDeclaration<'ast>,
  ) {
    if let Some(decl) = &named_decl.declaration {
      self
        .ctx
        .remove_node(Span::new(named_decl.span.start, decl.span().start));
      self.visit_declaration(decl);
    } else {
      self.ctx.remove_node(named_decl.span);
    }
  }

  fn visit_export_all_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportAllDeclaration<'ast>,
  ) {
    self.ctx.remove_node(decl.span);
  }

  fn visit_export_default_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportDefaultDeclaration<'ast>,
  ) {
    match &decl.declaration {
      oxc::ast::ast::ExportDefaultDeclarationKind::Expression(exp) => {
        if let Some(name) = self.ctx.default_symbol_name {
          self
            .ctx
            .overwrite(decl.span.start, exp.span().start, format!("var {name} = "));
        }
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(decl) => {
        self
          .ctx
          .remove_node(Span::new(decl.span.start, decl.span.start));
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(decl) => {
        self
          .ctx
          .remove_node(Span::new(decl.span.start, decl.span.start));
      }
      _ => {}
    }
  }

  fn visit_import_expression(&mut self, expr: &oxc::ast::ast::ImportExpression<'ast>) {
    if let oxc::ast::ast::Expression::StringLiteral(str) = &expr.source {
      let rec =
        &self.ctx.module.import_records[self.ctx.module.imports.get(&expr.span).copied().unwrap()];

      if let Some(chunk_id) = self.ctx.module_to_chunk[rec.resolved_module] {
        let chunk = &self.ctx.chunks[chunk_id];
        self.ctx.overwrite(
          str.span.start,
          str.span.end,
          // TODO: the path should be relative to the current importer chunk
          format!("'./{}'", chunk.file_name.as_ref().unwrap()),
        );
      } else {
        // external module doesn't belong to any chunk, just keep this as it is
      }
    }
  }
}
