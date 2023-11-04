use oxc::{
  ast::{ast::ExportDefaultDeclarationKind, Visit},
  span::GetSpan,
};
use rolldown_common::ExportsKind;

use crate::bundler::{module::Module, renderer::RenderControl};

use super::AstRenderer;

impl<'ast, 'r> Visit<'ast> for AstRenderer<'r> {
  fn visit_binding_identifier(&mut self, ident: &'ast oxc::ast::ast::BindingIdentifier) {
    self.render_binding_identifier(ident);
  }

  fn visit_call_expression(&mut self, expr: &'ast oxc::ast::ast::CallExpression<'ast>) {
    match &expr.callee {
      oxc::ast::ast::Expression::Identifier(callee_ident)
        if callee_ident.name == "require" || callee_ident.reference_id.get().is_none() =>
      {
        self.render_require_expr(expr);
        return;
      }
      _ => {}
    }

    // visit children
    for arg in &expr.arguments {
      self.visit_argument(arg);
    }

    // `IdentifierReference` in callee position need to be handled specially
    if let oxc::ast::ast::Expression::Identifier(s) = &expr.callee {
      self.render_identifier_reference(s, true);
    } else {
      self.visit_expression(&expr.callee);
    }

    if let Some(parameters) = &expr.type_parameters {
      self.visit_ts_type_parameter_instantiation(parameters);
    }
  }

  fn visit_identifier_reference(&mut self, ident: &'ast oxc::ast::ast::IdentifierReference) {
    self.render_identifier_reference(ident, false);
  }

  fn visit_import_declaration(&mut self, decl: &'ast oxc::ast::ast::ImportDeclaration<'ast>) {
    self.remove_node(decl.span);
    let module_id = self.ctx.module.get_import_module_by_span(decl.span);
    let importee = &self.ctx.graph.modules[module_id];
    let importee_linking_info = &self.ctx.graph.linking_infos[module_id];
    let Module::Normal(importee) = importee else { return };

    if importee.exports_kind == ExportsKind::CommonJs {
      self.hoisted_module_declaration(
        decl.span.start,
        self.ctx.generate_import_commonjs_module(
          importee,
          &self.ctx.graph.linking_infos[importee.id],
          true,
        ),
      );
    } else if let Some(wrap_ref) = importee_linking_info.wrap_ref {
      let wrap_ref_name = self.canonical_name_for(wrap_ref);
      // init wrapped esm module
      self.hoisted_module_declaration(decl.span.start, format!("{wrap_ref_name}();\n"));
    }
  }

  fn visit_export_all_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportAllDeclaration<'ast>,
  ) {
    if let Module::Normal(importee) = self.get_importee_by_span(decl.span) {
      if importee.exports_kind == ExportsKind::CommonJs {
        // __reExport(a_exports, __toESM(require_c()));
        let namespace_name = &self.ctx.canonical_names[&self.ctx.module.namespace_symbol];
        let re_export_runtime_symbol_name = self.canonical_name_for_runtime("__reExport");
        self.hoisted_module_declaration(
          decl.span.start,
          format!(
            "{re_export_runtime_symbol_name}({namespace_name}, {});",
            self.ctx.generate_import_commonjs_module(
              importee,
              &self.ctx.graph.linking_infos[importee.id],
              false
            )
          ),
        );
      }
    }
    self.remove_node(decl.span);
  }

  fn visit_import_expression(&mut self, expr: &oxc::ast::ast::ImportExpression<'ast>) {
    if let oxc::ast::ast::Expression::StringLiteral(str) = &expr.source {
      if let Some(chunk_id) =
        self.ctx.chunk_graph.module_to_chunk[self.ctx.module.get_import_module_by_span(expr.span)]
      {
        let chunk = &self.ctx.chunk_graph.chunks[chunk_id];
        self.overwrite(
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

  fn visit_statement(&mut self, stmt: &'ast oxc::ast::ast::Statement<'ast>) {
    // Mark the start position for place hoisted module declarations.
    if self.ctx.first_stmt_start.is_none() {
      let hoisted_decl = if let oxc::ast::ast::Statement::ModuleDeclaration(decl) = stmt {
        match &decl.0 {
          oxc::ast::ast::ModuleDeclaration::ImportDeclaration(_)
          | oxc::ast::ast::ModuleDeclaration::ExportAllDeclaration(_) => true,
          oxc::ast::ast::ModuleDeclaration::ExportNamedDeclaration(decl) => decl.source.is_some(),
          _ => false,
        }
      } else {
        false
      };
      if !hoisted_decl {
        self.ctx.first_stmt_start = Some(stmt.span().start);
      }
    }

    if self.try_render_require_statement(stmt).is_skip() {
      return;
    }

    // visit children
    self.visit_statement_match(stmt);
  }

  fn visit_export_named_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportNamedDeclaration<'ast>,
  ) {
    let control = match &mut self.kind {
      super::RenderKind::WrappedEsm => self.render_export_named_declaration_for_wrapped_esm(decl),
      super::RenderKind::Cjs => RenderControl::Continue,
      super::RenderKind::Esm => self.render_export_named_declaration_for_esm(decl),
    };

    if control.is_skip() {
      return;
    }
    // visit children
    if let Some(decl) = &decl.declaration {
      self.visit_declaration(decl);
    }
  }

  fn visit_export_default_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportDefaultDeclaration<'ast>,
  ) {
    let control = match &mut self.kind {
      super::RenderKind::WrappedEsm => self.render_export_default_declaration_for_wrapped_esm(decl),
      super::RenderKind::Cjs | super::RenderKind::Esm => self.strip_export_keyword(decl),
    };

    if control.is_skip() {
      return;
    }

    // visit children

    match &decl.declaration {
      ExportDefaultDeclarationKind::Expression(expr) => self.visit_expression(expr),
      ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
        self.visit_function(func);
      }
      ExportDefaultDeclarationKind::ClassDeclaration(class) => self.visit_class(class),
      _ => {}
    }
  }
}
