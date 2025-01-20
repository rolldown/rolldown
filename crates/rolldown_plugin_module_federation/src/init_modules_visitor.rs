use std::vec;

use oxc::{
  ast::{
    ast::{
      ExportAllDeclaration, ExportNamedDeclaration, FormalParameterKind, ImportDeclaration,
      ImportOrExportKind, Statement, VariableDeclarationKind,
    },
    AstBuilder, VisitMut, NONE,
  },
  span::SPAN,
};
use rolldown_utils::{concat_string, ecmascript::legitimize_identifier_name};

use crate::{utils::is_remote_module, ModuleFederationPluginOption};

const INIT_MODULE: &str = "__mf__init__module__";

pub struct InitModuleVisitor<'ast, 'a> {
  pub ast_builder: AstBuilder<'ast>,
  pub options: &'a ModuleFederationPluginOption,
  pub statements: Vec<Statement<'ast>>,
}

impl InitModuleVisitor<'_, '_> {
  #[allow(clippy::too_many_lines)]
  pub fn detect_static_module_decl(&mut self, request: &str) {
    if is_remote_module(request, self.options) {
      // import * as ns from 'app/App'
      let namespace = legitimize_identifier_name(request);
      let new_namespace = concat_string!("_mf_", namespace);
      let import_module = Statement::from(self.ast_builder.module_declaration_import_declaration(
        SPAN,
        Some(self.ast_builder.vec1(
          self.ast_builder.import_declaration_specifier_import_namespace_specifier(
            SPAN,
            self.ast_builder.binding_identifier(SPAN, namespace.as_ref()),
          ),
        )),
        self.ast_builder.string_literal(SPAN, self.ast_builder.atom(request), None),
        None,
        NONE,
        ImportOrExportKind::Value,
      ));

      // const nss = ns
      let assign_statement = self
        .ast_builder
        .declaration_variable(
          SPAN,
          VariableDeclarationKind::Const,
          self.ast_builder.vec1(
            self.ast_builder.variable_declarator(
              SPAN,
              VariableDeclarationKind::Const,
              self.ast_builder.binding_pattern(
                self
                  .ast_builder
                  .binding_pattern_kind_binding_identifier(SPAN, new_namespace.to_string()),
                NONE,
                false,
              ),
              Some(self.ast_builder.expression_identifier_reference(SPAN, namespace.as_ref())),
              false,
            ),
          ),
          false,
        )
        .into();

      // TODO: module.exports is not object
      // await ns.__mf__init__module__().then((m) => Object.assign(nss, m))
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
                self.ast_builder.expression_call(
                  SPAN,
                  self
                    .ast_builder
                    .member_expression_static(
                      SPAN,
                      self.ast_builder.expression_identifier_reference(SPAN, namespace.as_ref()),
                      self.ast_builder.identifier_name(SPAN, INIT_MODULE),
                      false,
                    )
                    .into(),
                  NONE,
                  self.ast_builder.vec(),
                  false,
                ),
                self.ast_builder.identifier_name(SPAN, "then"),
                false,
              )
              .into(),
            NONE,
            self.ast_builder.vec1(
              self
                .ast_builder
                .expression_arrow_function(
                  SPAN,
                  true,
                  false,
                  NONE,
                  self.ast_builder.formal_parameters(
                    SPAN,
                    FormalParameterKind::ArrowFormalParameters,
                    self.ast_builder.vec1(self.ast_builder.formal_parameter(
                      SPAN,
                      self.ast_builder.vec(),
                      self.ast_builder.binding_pattern(
                        self.ast_builder.binding_pattern_kind_binding_identifier(SPAN, "m"),
                        NONE,
                        false,
                      ),
                      None,
                      false,
                      false,
                    )),
                    NONE,
                  ),
                  NONE,
                  self.ast_builder.function_body(
                    SPAN,
                    self.ast_builder.vec(),
                    self.ast_builder.vec1(
                      self.ast_builder.statement_expression(
                        SPAN,
                        self.ast_builder.expression_call(
                          SPAN,
                          self
                            .ast_builder
                            .member_expression_static(
                              SPAN,
                              self.ast_builder.expression_identifier_reference(SPAN, "Object"),
                              self.ast_builder.identifier_name(SPAN, "assign"),
                              false,
                            )
                            .into(),
                          NONE,
                          {
                            let mut items = self.ast_builder.vec_with_capacity(2);
                            items.push(
                              self
                                .ast_builder
                                .expression_identifier_reference(SPAN, new_namespace)
                                .into(),
                            );
                            items.push(
                              self.ast_builder.expression_identifier_reference(SPAN, "m").into(),
                            );
                            items
                          },
                          false,
                        ),
                      ),
                    ),
                  ),
                )
                .into(),
            ),
            false,
          ),
        ),
      );
      self.statements.extend(vec![import_module, assign_statement, init_statement]);
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
