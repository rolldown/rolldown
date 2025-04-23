use oxc::{
  ast::{
    AstBuilder, NONE,
    ast::{Argument, Expression, ImportOrExportKind, PropertyKind, Statement},
  },
  ast_visit::VisitMut,
  span::{SPAN, Span},
  syntax::number::NumberBase,
};

use crate::DYNAMIC_IMPORT_HELPER;

use super::parse_pattern::{DynamicImportPattern, DynamicImportRequest, parse_pattern};
use super::to_glob::to_glob_pattern;

pub struct DynamicImportVarsVisit<'ast> {
  pub ast_builder: AstBuilder<'ast>,
  pub need_helper: bool,
}

impl<'ast> VisitMut<'ast> for DynamicImportVarsVisit<'ast> {
  #[allow(clippy::too_many_lines)]
  fn visit_expression(&mut self, expr: &mut Expression<'ast>) {
    if let Expression::ImportExpression(import_expr) = expr {
      // TODO: Support @/path via options.createResolver
      // TODO: handle error
      let pattern = to_glob_pattern(&import_expr.source).unwrap();
      if let Some(pattern) = pattern {
        let DynamicImportPattern { glob_params, user_pattern, raw_pattern: _ } =
          parse_pattern(pattern.as_str());
        self.need_helper = true;
        *expr = self.call_helper(
          import_expr.span,
          user_pattern.as_str(),
          std::mem::replace(
            &mut import_expr.source,
            self.ast_builder.expression_null_literal(SPAN),
          ),
          glob_params,
        );
      }
    }
  }
}

impl<'ast> DynamicImportVarsVisit<'ast> {
  /// generates:
  /// ```js
  /// __variableDynamicImportRuntimeHelper((import.meta.glob(pattern, params)), expr, segments)
  /// ```
  #[allow(clippy::cast_precision_loss)]
  fn call_helper(
    &self,
    span: Span,
    pattern: &str,
    expr: Expression<'ast>,
    params: Option<DynamicImportRequest>,
  ) -> Expression<'ast> {
    let segments = pattern.split('/').count();
    self.ast_builder.expression_call(
      span,
      self.ast_builder.expression_identifier(SPAN, "__variableDynamicImportRuntimeHelper"),
      NONE,
      {
        let mut items = self.ast_builder.vec();
        items.push(Argument::from(self.ast_builder.expression_parenthesized(
          SPAN,
          self.ast_builder.expression_call(
            SPAN,
            Expression::from(self.ast_builder.member_expression_static(
              SPAN,
              self.ast_builder.expression_meta_property(
                SPAN,
                self.ast_builder.identifier_name(SPAN, "import"),
                self.ast_builder.identifier_name(SPAN, "meta"),
              ),
              self.ast_builder.identifier_name(SPAN, "glob"),
              false,
            )),
            NONE,
            {
              let mut arguments = self.ast_builder.vec1(Argument::from(
                self.ast_builder.expression_string_literal(SPAN, pattern, None),
              ));
              if let Some(params) = params {
                arguments.push(Argument::from(self.ast_builder.expression_object(SPAN, {
                  let mut items =
                    self.ast_builder.vec1(self.ast_builder.object_property_kind_object_property(
                      SPAN,
                      PropertyKind::Init,
                      self.ast_builder.property_key_static_identifier(SPAN, "query"),
                      self.ast_builder.expression_string_literal(SPAN, params.query, None),
                      false,
                      false,
                      false,
                    ));
                  if params.import {
                    items.push(self.ast_builder.object_property_kind_object_property(
                      SPAN,
                      PropertyKind::Init,
                      self.ast_builder.property_key_static_identifier(SPAN, "import"),
                      self.ast_builder.expression_string_literal(SPAN, "*", None),
                      false,
                      false,
                      false,
                    ));
                  }
                  items
                })));
              }
              arguments
            },
            false,
          ),
        )));
        items.push(Argument::from(expr));
        items.push(Argument::from(self.ast_builder.expression_numeric_literal(
          SPAN,
          segments as f64,
          None,
          NumberBase::Decimal,
        )));
        items
      },
      false,
    )
  }

  /// generates:
  /// ```js
  /// import __variableDynamicImportRuntimeHelper from "${dynamicImportHelperId}";
  /// ```
  pub fn import_helper(&self) -> Statement<'ast> {
    Statement::from(self.ast_builder.module_declaration_import_declaration(
      SPAN,
      Some(self.ast_builder.vec1(
        self.ast_builder.import_declaration_specifier_import_default_specifier(
          SPAN,
          self.ast_builder.binding_identifier(SPAN, "__variableDynamicImportRuntimeHelper"),
        ),
      )),
      self.ast_builder.string_literal(SPAN, DYNAMIC_IMPORT_HELPER, None),
      None,
      NONE,
      ImportOrExportKind::Value,
    ))
  }
}
