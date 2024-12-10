use oxc::{
  ast::{
    ast::{Argument, Expression, ImportOrExportKind, PropertyKind, Statement},
    AstBuilder, VisitMut, NONE,
  },
  span::{Span, SPAN},
  syntax::number::NumberBase,
};
use parse_pattern::{parse_pattern, DynamicImportPattern, DynamicImportRequest};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformAstArgs, HookTransformAstReturn, Plugin, PluginContext,
};
use std::borrow::Cow;
use to_glob::to_glob_pattern;
mod parse_pattern;
mod should_ignore;
mod to_glob;

const DYNAMIC_IMPORT_HELPER: &str = "\0rolldown_dynamic_import_helper.js";

#[derive(Debug)]
pub struct DynamicImportVarsPlugin {}

impl Plugin for DynamicImportVarsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:dynamic_import_vars")
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == DYNAMIC_IMPORT_HELPER {
      Ok(Some(HookResolveIdOutput { id: DYNAMIC_IMPORT_HELPER.to_string(), ..Default::default() }))
    } else {
      Ok(None)
    }
  }

  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if args.id == DYNAMIC_IMPORT_HELPER {
      Ok(Some(HookLoadOutput {
        code: include_str!("dynamic_import_helper.js").to_string(),
        ..Default::default()
      }))
    } else {
      Ok(None)
    }
  }

  fn transform_ast(
    &self,
    _ctx: &PluginContext,
    mut args: HookTransformAstArgs,
  ) -> HookTransformAstReturn {
    // TODO: Ignore if includes a marker like "/* @rolldown-ignore */"
    args.ast.program.with_mut(|fields| {
      let ast_builder: AstBuilder = AstBuilder::new(fields.allocator);
      let mut visitor = DynamicImportVarsVisit { ast_builder, need_helper: false };
      visitor.visit_program(fields.program);
      if visitor.need_helper {
        fields.program.body.push(visitor.import_helper());
      }
    });
    Ok(args.ast)
  }
}

pub struct DynamicImportVarsVisit<'ast> {
  ast_builder: AstBuilder<'ast>,
  need_helper: bool,
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
      self
        .ast_builder
        .expression_identifier_reference(SPAN, "__variableDynamicImportRuntimeHelper"),
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
                arguments.push(Argument::from(self.ast_builder.expression_object(
                  SPAN,
                  {
                    let mut items =
                      self.ast_builder.vec1(self.ast_builder.object_property_kind_object_property(
                        SPAN,
                        PropertyKind::Init,
                        self.ast_builder.property_key_identifier_name(SPAN, "query"),
                        self.ast_builder.expression_string_literal(SPAN, params.query, None),
                        false,
                        false,
                        false,
                      ));
                    if params.import {
                      items.push(self.ast_builder.object_property_kind_object_property(
                        SPAN,
                        PropertyKind::Init,
                        self.ast_builder.property_key_identifier_name(SPAN, "import"),
                        self.ast_builder.expression_string_literal(SPAN, "*", None),
                        false,
                        false,
                        false,
                      ));
                    }
                    items
                  },
                  None,
                )));
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
  fn import_helper(&self) -> Statement<'ast> {
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
