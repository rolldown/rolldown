use clone_expr::clone_expr;
use oxc::{
  ast::{
    ast::{Expression, ImportOrExportKind, Statement, TSTypeParameterInstantiation},
    AstBuilder, VisitMut,
  },
  span::SPAN,
};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformAstArgs, HookTransformAstReturn, Plugin, SharedPluginContext,
};
use std::borrow::Cow;
use to_glob::to_glob_pattern;
mod clone_expr;
mod should_ignore;
mod to_glob;

const DYNAMIC_IMPORT_HELPER_ID: &str = "\0rolldown/dynamic-import-helper.js";

#[derive(Debug)]
pub struct DynamicImportVarsPlugin {}

impl Plugin for DynamicImportVarsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("dynamic_import_vars")
  }

  async fn resolve_id(
    &self,
    _ctx: &SharedPluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == DYNAMIC_IMPORT_HELPER_ID {
      Ok(Some(HookResolveIdOutput {
        id: DYNAMIC_IMPORT_HELPER_ID.to_string(),
        external: Some(true),
        ..Default::default()
      }))
    } else {
      Ok(None)
    }
  }

  async fn load(&self, _ctx: &SharedPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if args.id == DYNAMIC_IMPORT_HELPER_ID {
      Ok(Some(HookLoadOutput {
        code: include_str!("./dynamic_import_helper.js").to_string(),
        ..Default::default()
      }))
    } else {
      Ok(None)
    }
  }

  fn transform_ast(
    &self,
    _ctx: &SharedPluginContext,
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
      let pattern = to_glob_pattern(&import_expr.source).unwrap();
      if let Some(pattern) = pattern {
        self.need_helper = true;
        *expr = self.call_helper(pattern.as_str(), &import_expr.source);
      }
    }
  }
}

impl<'ast> DynamicImportVarsVisit<'ast> {
  /// generates:
  /// ```js
  /// __variableDynamicImportRuntimeHelper((import.meta.glob(pattern)), expr)
  /// ```
  fn call_helper(&self, pattern: &str, expr: &Expression<'ast>) -> Expression<'ast> {
    self.ast_builder.expression_call(
      SPAN,
      {
        let mut items = self.ast_builder.vec();
        items.push(
          self.ast_builder.argument_expression(
            self.ast_builder.expression_parenthesized(
              SPAN,
              self.ast_builder.expression_call(
                SPAN,
                self.ast_builder.vec1(
                  self
                    .ast_builder
                    .argument_expression(self.ast_builder.expression_string_literal(SPAN, pattern)),
                ),
                self.ast_builder.expression_member(self.ast_builder.member_expression_static(
                  SPAN,
                  self.ast_builder.expression_meta_property(
                    SPAN,
                    self.ast_builder.identifier_name(SPAN, "import"),
                    self.ast_builder.identifier_name(SPAN, "meta"),
                  ),
                  self.ast_builder.identifier_name(SPAN, "glob"),
                  false,
                )),
                Option::<TSTypeParameterInstantiation>::None,
                false,
              ),
            ),
          ),
        );
        items.push(self.ast_builder.argument_expression(clone_expr(self.ast_builder, expr)));
        items
      },
      self
        .ast_builder
        .expression_identifier_reference(SPAN, "__variableDynamicImportRuntimeHelper"),
      Option::<TSTypeParameterInstantiation>::None,
      false,
    )
  }

  /// generates:
  /// ```js
  /// import __variableDynamicImportRuntimeHelper from "${dynamicImportHelperId}";
  /// ```
  fn import_helper(&self) -> Statement<'ast> {
    self.ast_builder.statement_module_declaration(
      self.ast_builder.module_declaration_import_declaration(
        SPAN,
        Some(self.ast_builder.vec1(
          self.ast_builder.import_declaration_specifier_import_default_specifier(
            SPAN,
            self.ast_builder.binding_identifier(SPAN, "__variableDynamicImportRuntimeHelper"),
          ),
        )),
        self.ast_builder.string_literal(SPAN, DYNAMIC_IMPORT_HELPER_ID),
        None,
        ImportOrExportKind::Value,
      ),
    )
  }
}
