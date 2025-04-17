use std::sync::Arc;

use oxc::{
  allocator::CloneIn,
  ast::{
    AstBuilder, NONE,
    ast::{Argument, ExportAllDeclaration, ExportNamedDeclaration, Expression, ImportDeclaration},
  },
  ast_visit::{VisitMut, walk_mut::walk_expression},
  span::SPAN,
};
use rolldown_utils::concat_string;
use rustc_hash::FxHashSet;

use crate::{
  ModuleFederationPluginOption,
  utils::{RemoteModuleType, detect_remote_module_type, get_remote_module_prefix},
};

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct RemoteModule {
  pub id: Arc<str>,
  pub r#type: RemoteModuleType,
}

pub struct InitModuleVisitor<'ast, 'a> {
  #[allow(dead_code)]
  pub ast_builder: AstBuilder<'ast>,
  pub options: &'a ModuleFederationPluginOption,
  pub init_remote_modules: &'a mut FxHashSet<RemoteModule>,
}

impl InitModuleVisitor<'_, '_> {
  #[allow(clippy::too_many_lines)]
  pub fn detect_static_module_decl(&mut self, request: &str) {
    if let Some(remote_type) = detect_remote_module_type(request, self.options) {
      self.init_remote_modules.insert(RemoteModule { id: request.into(), r#type: remote_type });
    }
  }
}

// TODO require
impl<'ast> VisitMut<'ast> for InitModuleVisitor<'ast, '_> {
  fn visit_import_declaration(&mut self, decl: &mut ImportDeclaration<'ast>) {
    self.detect_static_module_decl(&decl.source.value);
  }

  fn visit_export_all_declaration(&mut self, decl: &mut ExportAllDeclaration<'ast>) {
    self.detect_static_module_decl(&decl.source.value);
  }

  fn visit_export_named_declaration(&mut self, decl: &mut ExportNamedDeclaration<'ast>) {
    if let Some(source) = &decl.source {
      self.detect_static_module_decl(&source.value);
    }
  }

  fn visit_expression(&mut self, expr: &mut Expression<'ast>) {
    // import('module') => import('init_module_module').then(() => import('module'))
    if let Expression::ImportExpression(import_expr) = expr {
      if let Expression::StringLiteral(lit) = &import_expr.source {
        if let Some(remote_type) = detect_remote_module_type(&lit.value, self.options) {
          let id = concat_string!(get_remote_module_prefix(remote_type), lit.value.as_str());
          *expr = self.ast_builder.expression_call(
            SPAN,
            self
              .ast_builder
              .member_expression_static(
                SPAN,
                self.ast_builder.expression_import(
                  SPAN,
                  self.ast_builder.expression_string_literal(
                    SPAN,
                    self.ast_builder.atom(&id),
                    None,
                  ),
                  None,
                  None,
                ),
                self.ast_builder.identifier_name(SPAN, self.ast_builder.atom("then")),
                false,
              )
              .into(),
            NONE,
            self.ast_builder.vec1(Argument::ArrowFunctionExpression(
              self.ast_builder.alloc_arrow_function_expression(
                SPAN,
                true,
                false,
                NONE,
                self.ast_builder.formal_parameters(
                  SPAN,
                  oxc::ast::ast::FormalParameterKind::Signature,
                  self.ast_builder.vec(),
                  NONE,
                ),
                NONE,
                self.ast_builder.function_body(
                  SPAN,
                  self.ast_builder.vec(),
                  self.ast_builder.vec1(self.ast_builder.statement_expression(
                    SPAN,
                    Expression::ImportExpression(import_expr.clone_in(self.ast_builder.allocator)),
                  )),
                ),
              ),
            )),
            false,
          );
          return;
        }
      }
    }
    walk_expression(self, expr);
  }
}
