use std::vec;

use oxc::{
  ast::{
    ast::{
      ExportAllDeclaration, ExportNamedDeclaration, ImportDeclaration, ImportOrExportKind,
      Statement,
    },
    AstBuilder, VisitMut, NONE,
  },
  span::SPAN,
};
use rolldown_utils::ecmascript::legitimize_identifier_name;

use crate::{utils::is_remote_module, ModuleFederationPluginOption};

const INIT_MODULE: &str = "__mf__init__module__";

pub struct InitModuleVisitor<'ast, 'a> {
  pub ast_builder: AstBuilder<'ast>,
  pub options: &'a ModuleFederationPluginOption,
  pub statements: Vec<Statement<'ast>>,
}

impl InitModuleVisitor<'_, '_> {
  pub fn detect_static_module_decl(&mut self, request: &str) {
    if is_remote_module(request, self.options) {
      // import * as ns from 'app/App'
      // await ns.__mf__init__module__()
      let name = legitimize_identifier_name(request);
      let import_module = Statement::from(self.ast_builder.module_declaration_import_declaration(
        SPAN,
        Some(self.ast_builder.vec1(
          self.ast_builder.import_declaration_specifier_import_namespace_specifier(
            SPAN,
            self.ast_builder.binding_identifier(SPAN, name.as_ref()),
          ),
        )),
        self.ast_builder.string_literal(SPAN, self.ast_builder.atom(request), None),
        None,
        NONE,
        ImportOrExportKind::Value,
      ));

      let init_statement = self.ast_builder.statement_expression(
        SPAN,
        self.ast_builder.expression_await(
          SPAN,
          self.ast_builder.expression_call(
            SPAN,
            self
              .ast_builder
              .member_expression_static(
                SPAN,
                self.ast_builder.expression_identifier_reference(SPAN, name),
                self.ast_builder.identifier_name(SPAN, INIT_MODULE),
                false,
              )
              .into(),
            NONE,
            self.ast_builder.vec(),
            false,
          ),
        ),
      );
      self.statements.extend(vec![import_module, init_statement]);
    }
  }
}

// TODO require/ import()
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
}
