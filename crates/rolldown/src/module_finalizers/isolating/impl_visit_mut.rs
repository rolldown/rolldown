use oxc::ast::ast::{self, ExportDefaultDeclarationKind, Expression, Statement};
use oxc::ast_visit::{VisitMut, walk_mut};
use oxc::span::{CompactStr, SPAN, Span};
use rolldown_common::{Interop, Module, SymbolRef};
use rolldown_ecmascript_utils::{CallExpressionExt, TakeIn};

use super::IsolatingModuleFinalizer;

impl<'ast> VisitMut<'ast> for IsolatingModuleFinalizer<'_, 'ast> {
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    // Drop the hashbang since we already store them in ast_scan phase and
    // we don't want oxc to generate hashbang statement in module level since we already handle
    // them in chunk level
    program.hashbang.take();
    let mut stmts = self.snippet.builder.vec();

    for mut stmt in program.body.take_in(self.alloc) {
      walk_mut::walk_statement(self, &mut stmt);
      match &mut stmt {
        Statement::ImportDeclaration(import_decl) => {
          self.transform_import_declaration(import_decl);
        }
        ast::Statement::ExportDefaultDeclaration(export_default_decl) => {
          stmts.push(self.transform_export_default_declaration(export_default_decl));
        }
        ast::Statement::ExportNamedDeclaration(export_named_decl) => {
          if let Some(stmt) = self.transform_named_declaration(export_named_decl) {
            stmts.push(stmt);
          }
        }
        ast::Statement::ExportAllDeclaration(export_all_decl) => {
          self.transform_export_all_declaration(export_all_decl);
        }
        _ => stmts.push(stmt),
      }
    }

    // Add __esModule flag for esm module
    if self.ctx.module.exports_kind.is_esm() {
      program.body.push(self.snippet.builder.statement_expression(
        SPAN,
        self.snippet.call_expr_with_arg_expr(
          self.snippet.id_ref_expr("__toCommonJS", SPAN),
          self.snippet.id_ref_expr("exports", SPAN),
          false,
        ),
      ));
    }

    // Generate export statements, using `Object.defineProperty`
    if !self.generated_exports.is_empty() {
      program.body.push(self.snippet.builder.statement_expression(
        SPAN,
        self.snippet.alloc_call_expr_with_2arg_expr_expr(
          "__export",
          self.snippet.id_ref_expr("exports", SPAN),
          Expression::ObjectExpression(self.snippet.builder.alloc_object_expression(
            SPAN,
            self.snippet.builder.vec_from_iter(self.generated_exports.drain(..)),
            None,
          )),
        ),
      ));
    }

    // Add generated imports
    program.body.extend(self.generated_imports.drain(..));

    program.body.extend(stmts);
  }

  fn visit_expression(&mut self, expr: &mut Expression<'ast>) {
    if let Expression::Identifier(ident) = expr {
      if let Some(named_import) = ident
        .reference_id
        .get()
        .and_then(|reference_id| self.scope.symbol_id_for(reference_id))
        .map(|symbol_id| (self.ctx.module.idx, symbol_id).into())
        .and_then(|symbol_ref: SymbolRef| self.ctx.module.named_imports.get(&symbol_ref))
      {
        let rec = &self.ctx.module.import_records[named_import.record_id];

        let namespace_object_ref =
          self.create_namespace_object_ref_for_module(&self.ctx.modules[rec.resolved_module]);

        match &named_import.imported {
          rolldown_common::Specifier::Star => {
            ident.name = self.snippet.atom(namespace_object_ref.as_str());
          }
          rolldown_common::Specifier::Literal(imported) => {
            *expr = Expression::StaticMemberExpression(
              self.snippet.builder.alloc_static_member_expression(
                ident.span,
                self.snippet.id_ref_expr(namespace_object_ref.as_str(), SPAN),
                self.snippet.builder.identifier_name(SPAN, imported.as_str()),
                false,
              ),
            );
          }
        }
      }
    }
    walk_mut::walk_expression(self, expr);
  }

  fn visit_call_expression(&mut self, expr: &mut ast::CallExpression<'ast>) {
    if expr.is_global_require_call(self.scope) {
      if let Some(ast::Argument::StringLiteral(request)) = expr.arguments.first_mut() {
        request.value = self.snippet.atom(self.get_importee_module(expr.span).stable_id());
      }
    }

    walk_mut::walk_call_expression(self, expr);
  }
}

impl<'ast> IsolatingModuleFinalizer<'_, 'ast> {
  pub fn transform_import_declaration(&mut self, import_decl: &ast::ImportDeclaration<'ast>) {
    // The specifiers rewrite with reference the namespace object, see `IsolatingModuleFinalizer#visit_expression`

    // Create a require call statement for import declaration
    let module = self.get_importee_module(import_decl.span);
    let namespace_object_ref = self.create_namespace_object_ref_for_module(module);
    self.create_require_call_stmt(
      &module.stable_id().into(),
      self.get_interop(module),
      &namespace_object_ref,
      import_decl.span,
    );
  }

  pub fn transform_export_default_declaration(
    &mut self,
    export_default_decl: &mut ast::ExportDefaultDeclaration<'ast>,
  ) -> Statement<'ast> {
    // TODO deconflict default_export_ref
    let default_export_ref = self.ctx.module.default_export_ref.name(self.ctx.symbol_db);

    match &mut export_default_decl.declaration {
      decl @ ast::match_expression!(ExportDefaultDeclarationKind) => {
        self.generated_exports.push(self.snippet.object_property_kind_object_property(
          "default",
          self.snippet.id_ref_expr(default_export_ref, SPAN),
          false,
        ));
        self.snippet.var_decl_stmt(default_export_ref, decl.to_expression_mut().take_in(self.alloc))
      }
      ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
        let from = func.id.as_ref().map_or(default_export_ref, |ident| ident.name.as_str());
        self.generated_exports.push(self.snippet.object_property_kind_object_property(
          "default",
          self.snippet.id_ref_expr(from, SPAN),
          false,
        ));
        self
          .snippet
          .builder
          .statement_expression(SPAN, Expression::FunctionExpression(func.take_in(self.alloc)))
      }
      ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
        let from = class.id.as_ref().map_or(default_export_ref, |ident| ident.name.as_str());
        self.generated_exports.push(self.snippet.object_property_kind_object_property(
          "default",
          self.snippet.id_ref_expr(from, SPAN),
          false,
        ));
        self
          .snippet
          .builder
          .statement_expression(SPAN, Expression::ClassExpression(class.take_in(self.alloc)))
      }
      ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {
        unreachable!("ExportDefaultDeclaration TSInterfaceDeclaration should be removed")
      }
    }
  }

  #[allow(clippy::too_many_lines)]
  pub fn transform_named_declaration(
    &mut self,
    export_named_decl: &mut ast::ExportNamedDeclaration<'ast>,
  ) -> Option<Statement<'ast>> {
    match &export_named_decl.source {
      Some(_) => {
        let module = self.get_importee_module(export_named_decl.span);
        let namespace_object_ref = self.create_namespace_object_ref_for_module(module);
        self.create_require_call_stmt(
          &module.stable_id().into(),
          self.get_interop(module),
          &namespace_object_ref,
          export_named_decl.span,
        );

        self.generated_exports.extend(export_named_decl.specifiers.iter().map(|specifier| {
          self.snippet.object_property_kind_object_property(
            &specifier.exported.name(),
            match &specifier.local {
              ast::ModuleExportName::IdentifierName(ident) => {
                Expression::StaticMemberExpression(
                  self.snippet.builder.alloc_static_member_expression(
                    SPAN,
                    self.snippet.id_ref_expr(&namespace_object_ref, SPAN),
                    self.snippet.builder.identifier_name(SPAN, ident.name.as_str()),
                    false,
                  ),
                )
              }
              ast::ModuleExportName::StringLiteral(str) => {
                Expression::ComputedMemberExpression(
                  self.snippet.builder.alloc_computed_member_expression(
                    SPAN,
                    self.snippet.id_ref_expr(&namespace_object_ref, SPAN),
                    self.snippet.builder.expression_string_literal(
                      SPAN, str.value.as_str(), None
                    ),
                    false,
                  ),
                )
              }
              ast::ModuleExportName::IdentifierReference(_) => {
                unreachable!(
                  "ModuleExportName IdentifierReference is invalid in ExportNamedDeclaration with source"
                )
              }
            },
            matches!(specifier.exported, ast::ModuleExportName::StringLiteral(_))
          )
        }));
        None
      }
      None => {
        if let Some(decl) = &mut export_named_decl.declaration {
          match decl {
            ast::Declaration::VariableDeclaration(var_decl) => {
              self.generated_exports.extend(var_decl.declarations.iter().filter_map(|decl| {
                decl.id.get_identifier_name().map(|ident| {
                  self.snippet.object_property_kind_object_property(
                    ident.as_str(),
                    self.snippet.id_ref_expr(ident.as_str(), SPAN),
                    false,
                  )
                })
              }));

              return Some(ast::Statement::VariableDeclaration(
                self.snippet.builder.alloc_variable_declaration(
                  SPAN,
                  var_decl.kind,
                  var_decl.declarations.take_in(self.alloc),
                  false,
                ),
              ));
            }
            ast::Declaration::FunctionDeclaration(func_decl) => {
              let from =
                func_decl.id.as_ref().expect("FunctionDeclaration should have ident").name.as_str();
              self.generated_exports.push(self.snippet.object_property_kind_object_property(
                from,
                self.snippet.id_ref_expr(from, SPAN),
                false,
              ));
              return Some(self.snippet.builder.statement_expression(
                SPAN,
                Expression::FunctionExpression(func_decl.take_in(self.alloc)),
              ));
            }
            ast::Declaration::ClassDeclaration(class_decl) => {
              let from =
                class_decl.id.as_ref().expect("ClassDeclaration should have ident").name.as_str();
              self.generated_exports.push(self.snippet.object_property_kind_object_property(
                from,
                self.snippet.id_ref_expr(from, SPAN),
                false,
              ));
              return Some(self.snippet.builder.statement_expression(
                SPAN,
                Expression::ClassExpression(class_decl.take_in(self.alloc)),
              ));
            }
            _ => {}
          }
        }

        self.generated_exports.extend(export_named_decl.specifiers.iter().map(|specifier| {
          self.snippet.object_property_kind_object_property(
            &specifier.exported.name(),
            match &specifier.local {
              ast::ModuleExportName::IdentifierName(ident) => {
                self.snippet.id_ref_expr(ident.name.as_str(), SPAN)
              }
              ast::ModuleExportName::StringLiteral(_) => {
                unreachable!("ModuleExportName StringLiteral is invalid in ExportNamedDeclaration without source")
              }
              ast::ModuleExportName::IdentifierReference(ident) => {
                self.snippet.id_ref_expr(ident.name.as_str(), SPAN)
              }
            },
            matches!(specifier.exported, ast::ModuleExportName::StringLiteral(_)
          ))
        }));
        None
      }
    }
  }

  pub fn transform_export_all_declaration(
    &mut self,
    export_all_decl: &ast::ExportAllDeclaration<'ast>,
  ) {
    let module = self.get_importee_module(export_all_decl.span);
    let namespace_object_ref = self.create_namespace_object_ref_for_module(module);
    self.create_require_call_stmt(
      &module.stable_id().into(),
      self.get_interop(module),
      &namespace_object_ref,
      export_all_decl.span,
    );

    match &export_all_decl.exported {
      Some(exported) => {
        self.generated_exports.push(self.snippet.object_property_kind_object_property(
          &exported.name(),
          self.snippet.id_ref_expr(&namespace_object_ref, SPAN),
          matches!(exported, ast::ModuleExportName::StringLiteral(_)),
        ));
      }
      None => {
        self.generated_imports.push(self.snippet.builder.statement_expression(
          SPAN,
          self.snippet.call_expr_with_2arg_expr(
            self.snippet.id_ref_expr("__reExport", SPAN),
            self.snippet.id_ref_expr("exports", SPAN),
            self.snippet.id_ref_expr("namespace_object_ref", SPAN),
          ),
        ));
      }
    }
  }

  fn create_require_call_stmt(
    &mut self,
    module_stable_id: &CompactStr,
    interop: Option<Interop>,
    namespace_object_ref: &CompactStr,
    span: Span,
  ) {
    if self.generated_imports_set.contains(namespace_object_ref) {
      return;
    }

    self.generated_imports_set.insert(namespace_object_ref.clone());

    let require_call = self.snippet.require_call_expr(module_stable_id.as_str());

    self.generated_imports.push(self.snippet.variable_declarator_require_call_stmt(
      namespace_object_ref,
      self.snippet.to_esm_call_with_interop("__toESM", require_call, interop),
      span,
    ));
  }

  fn create_namespace_object_ref_for_module(&self, module: &Module) -> CompactStr {
    match module {
      Module::Normal(importee) => {
        // TODO deconflict namespace_ref
        importee.namespace_object_ref.name(self.ctx.symbol_db).into()
      }
      Module::External(external_module) => {
        // TODO need to generate one symbol and deconflict it
        external_module.identifier_name.as_str().into()
      }
    }
  }

  fn get_importee_module(&self, span: Span) -> &Module {
    let rec_id = self.ctx.module.imports[&span];
    let rec = &self.ctx.module.import_records[rec_id];
    &self.ctx.modules[rec.resolved_module]
  }

  fn get_interop(&self, importee: &Module) -> Option<Interop> {
    match importee {
      Module::Normal(importee) => self.ctx.module.interop(importee),
      Module::External(_) => None,
    }
  }
}
