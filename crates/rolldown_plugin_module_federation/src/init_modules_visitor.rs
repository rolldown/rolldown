use oxc::{
  ast::{
    ast::{
      ExportAllDeclaration, ExportNamedDeclaration, ImportDeclaration, ImportOrExportKind,
      Statement,
    },
    AstBuilder, VisitMut, NONE,
  },
  span::{Atom, SPAN},
};
use rustc_hash::FxHashSet;

use crate::{utils::is_remote_module, ModuleFederationPluginOption, REMOTE_MODULE_REGISTRY};

const LOAD_REMOTE_TO_REGISTRY: &str = "loadRemoteToRegistry";

pub struct InitModuleVisitor<'ast, 'a> {
  pub ast_builder: AstBuilder<'ast>,
  pub options: &'a ModuleFederationPluginOption,
  pub statements: Vec<Statement<'ast>>,
  pub insert_registry: bool,
  pub insert_init_remote_modules: FxHashSet<Atom<'ast>>,
}

impl InitModuleVisitor<'_, '_> {
  #[allow(clippy::too_many_lines)]
  pub fn detect_static_module_decl(&mut self, request: &str) {
    if is_remote_module(request, self.options) {
      // import { loadRemoteToRegistry } from 'remote-module-registry.js'
      if !self.insert_registry {
        let import_registry_statement =
          Statement::from(self.ast_builder.module_declaration_import_declaration(
            SPAN,
            Some(self.ast_builder.vec1(
              self.ast_builder.import_declaration_specifier_import_specifier(
                SPAN,
                self.ast_builder.module_export_name_identifier_name(SPAN, LOAD_REMOTE_TO_REGISTRY),
                self.ast_builder.binding_identifier(SPAN, LOAD_REMOTE_TO_REGISTRY),
                ImportOrExportKind::Value,
              ),
            )),
            self.ast_builder.string_literal(
              SPAN,
              self.ast_builder.atom(REMOTE_MODULE_REGISTRY),
              None,
            ),
            None,
            NONE,
            ImportOrExportKind::Value,
          ));
        self.statements.push(import_registry_statement);
        self.insert_registry = true;
      }

      // await loadRemoteToRegistry('app/App')
      if !self.insert_init_remote_modules.contains(request) {
        let init_module_statement = self.ast_builder.statement_expression(
          SPAN,
          self.ast_builder.expression_await(
            SPAN,
            self.ast_builder.expression_call(
              SPAN,
              self.ast_builder.expression_identifier_reference(SPAN, LOAD_REMOTE_TO_REGISTRY),
              NONE,
              self.ast_builder.vec1(
                self
                  .ast_builder
                  .expression_string_literal(SPAN, self.ast_builder.atom(request), None)
                  .into(),
              ),
              false,
            ),
          ),
        );
        self.statements.push(init_module_statement);
        self.insert_init_remote_modules.insert(self.ast_builder.atom(request));
      }
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
