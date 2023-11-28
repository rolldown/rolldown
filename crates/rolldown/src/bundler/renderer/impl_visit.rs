use oxc::{
  ast::{ast, ast::ExportDefaultDeclarationKind, Visit},
  span::GetSpan,
};
use rolldown_common::ExportsKind;
use rolldown_oxc::BindingIdentifierExt;
use rolldown_utils::MagicStringExt;

use crate::bundler::{module::Module, renderer::RenderControl};

use super::AstRenderer;

impl<'ast, 'r> AstRenderer<'r> {
  fn visit_top_level_stmt(&mut self, stmt: &ast::Statement<'ast>) {
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
    if self.current_stmt_info.get().is_included {
      self.visit_statement(stmt);
    } else {
      self.ctx.remove_stmt(stmt.span());
    }
  }
}

impl<'ast, 'r> Visit<'ast> for AstRenderer<'r> {
  #[tracing::instrument(skip_all)]
  fn visit_program(&mut self, program: &ast::Program<'ast>) {
    for directive in &program.directives {
      self.visit_directive(directive);
    }
    for (stmt_idx, stmt) in program.body.iter().enumerate() {
      self.current_stmt_info.next();
      debug_assert!(self.current_stmt_info.get().stmt_idx == Some(stmt_idx));
      self.visit_top_level_stmt(stmt);
    }
  }

  fn visit_binding_identifier(&mut self, ident: &oxc::ast::ast::BindingIdentifier) {
    self.render_binding_identifier(ident);
  }

  fn visit_call_expression(&mut self, expr: &oxc::ast::ast::CallExpression<'ast>) {
    if self.is_global_require(&expr.callee) {
      self.render_require_expr(expr);
      return;
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

  fn visit_identifier_reference(&mut self, ident: &oxc::ast::ast::IdentifierReference) {
    self.render_identifier_reference(ident, false);
  }

  fn visit_import_declaration(&mut self, decl: &oxc::ast::ast::ImportDeclaration<'ast>) {
    self.remove_node(decl.span);
    let importee_id = self.ctx.module.importee_id_by_span(decl.span);
    let importee = &self.ctx.graph.modules[importee_id];
    let importee_linking_info = &self.ctx.graph.linking_infos[importee_id];
    let Module::Normal(importee) = importee else { return };
    if importee.exports_kind == ExportsKind::CommonJs {
      self.hoisted_module_declaration(
        decl.span.start,
        self.ctx.generate_import_commonjs_module(
          &self.ctx.graph.linking_infos[importee.id],
          Some(decl.span),
        ),
      );
    } else if let Some(wrap_ref) = importee_linking_info.wrapper_ref {
      let wrap_ref_name = self.canonical_name_for(wrap_ref);
      // init wrapped esm module
      self.hoisted_module_declaration(decl.span.start, format!("{wrap_ref_name}();\n"));
    }
  }

  fn visit_export_all_declaration(&mut self, decl: &oxc::ast::ast::ExportAllDeclaration<'ast>) {
    if let Module::Normal(importee) = self.ctx.importee_by_span(decl.span) {
      if importee.exports_kind == ExportsKind::CommonJs {
        // __reExport(a_exports, __toESM(require_c()));
        let namespace_name = &self.ctx.canonical_names[&self.ctx.module.namespace_symbol];
        let re_export_runtime_symbol_name = self.canonical_name_for_runtime("__reExport");
        self.hoisted_module_declaration(
          decl.span.start,
          format!(
            "{re_export_runtime_symbol_name}({namespace_name}, {});",
            self
              .ctx
              .generate_import_commonjs_module(&self.ctx.graph.linking_infos[importee.id], None)
          ),
        );
      }
    }
    self.remove_node(decl.span);
  }

  fn visit_import_expression(&mut self, expr: &oxc::ast::ast::ImportExpression<'ast>) {
    if let oxc::ast::ast::Expression::StringLiteral(str) = &expr.source {
      let importee = self.ctx.module.importee_id_by_span(expr.span);
      match self.ctx.graph.modules[importee] {
        Module::Normal(_) => {
          debug_assert!(matches!(self.ctx.graph.modules[importee], Module::Normal(_)));
          let chunk_id = self.ctx.chunk_graph.module_to_chunk[importee]
            .expect("Normal module should belong to a chunk");
          let chunk = &self.ctx.chunk_graph.chunks[chunk_id];
          self.overwrite(
            str.span.start,
            str.span.end,
            // TODO: the path should be relative to the current importer chunk
            format!("'./{}'", chunk.file_name.as_ref().unwrap()),
          );
        }
        Module::External(_) => {
          // external module doesn't belong to any chunk, just keep this as it is
        }
      }
    }
  }

  fn visit_statement(&mut self, stmt: &oxc::ast::ast::Statement<'ast>) {
    if self.try_render_require_statement(stmt).is_skip() {
      return;
    }

    // visit children
    self.visit_statement_match(stmt);
  }

  fn visit_export_named_declaration(&mut self, decl: &oxc::ast::ast::ExportNamedDeclaration<'ast>) {
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
    decl: &oxc::ast::ast::ExportDefaultDeclaration<'ast>,
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
        self.visit_function(func, None);
      }
      ExportDefaultDeclarationKind::ClassDeclaration(class) => self.visit_class(class),
      _ => {}
    }
  }

  fn visit_object_pattern(&mut self, pat: &ast::ObjectPattern) {
    // visit children
    for prop in &pat.properties {
      match &prop.value.kind {
        // Rewrite `const { a } = obj;`` to `const { a: a$1 } = obj;`
        ast::BindingPatternKind::BindingIdentifier(ident) if prop.shorthand => {
          self.visit_property_key(&prop.key);

          match self.need_to_rename((self.ctx.module.id, ident.expect_symbol_id()).into()) {
            Some(new_name) if new_name != &ident.name => {
              self.ctx.source.overwrite(
                ident.span.start,
                ident.span.end,
                format!("{}: {new_name}", ident.name),
              );
            }
            _ => {}
          }
        }
        // Rewrite `const { a = 1 } = obj;`` to `const { a: a$1 = 1 } = obj;`
        ast::BindingPatternKind::AssignmentPattern(assign_pat)
          if prop.shorthand
            && matches!(assign_pat.left.kind, ast::BindingPatternKind::BindingIdentifier(_)) =>
        {
          let ast::BindingPatternKind::BindingIdentifier(ident) = &assign_pat.left.kind else {
            unreachable!()
          };
          match self.need_to_rename((self.ctx.module.id, ident.expect_symbol_id()).into()) {
            Some(new_name) if new_name != &ident.name => {
              self.ctx.source.overwrite(
                ident.span.start,
                ident.span.end,
                format!("{}: {new_name}", ident.name),
              );
            }
            _ => {}
          }
          self.visit_expression(&assign_pat.right);
        }
        _ => {
          self.visit_binding_property(prop);
        }
      }
    }
    if let Some(rest) = &pat.rest {
      self.visit_rest_element(rest);
    }
  }

  fn visit_assignment_pattern(&mut self, pat: &ast::AssignmentPattern) {
    // Visit children
    self.visit_binding_pattern(&pat.left);
    self.visit_expression(&pat.right);
  }
}
