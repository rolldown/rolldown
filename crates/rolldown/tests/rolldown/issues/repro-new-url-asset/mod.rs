use std::{borrow::Cow, path::Path, sync::Arc};

use oxc::{
  ast::{ast::Expression, AstBuilder, VisitMut},
  span::SPAN,
};
use rolldown::{BundlerOptions, InputItem};
use rolldown_common::EmittedAsset;
use rolldown_ecmascript::ExpressionExt;
use rolldown_plugin::{Plugin, SharedPluginContext};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

// cargo test -p rolldown --test integration_rolldown repro_new_url_asset

// https://github.com/vitejs/vite/blob/b3f5dfef8da92197e0d8eec0507f2c6ef7467418/packages/vite/src/node/plugins/assetImportMetaUrl.ts#L26
// https://github.com/web-infra-dev/rspack/blob/3867fe0279d0e2950ce8650ae56f3fd12fff1b04/crates/rspack_plugin_javascript/src/parser_plugin/url_plugin.rs#L51

#[derive(Debug)]
struct AssetImportMetaUrlPlugin {}

impl Plugin for AssetImportMetaUrlPlugin {
  fn name(&self) -> Cow<'static, str> {
    "asset-import-meta-url".into()
  }

  fn transform_ast(
    &self,
    ctx: &SharedPluginContext,
    mut args: rolldown_plugin::HookTransformAstArgs,
  ) -> rolldown_plugin::HookTransformAstReturn {
    args.ast.program.with_mut(|fields| {
      let ast_builder = AstBuilder::new(fields.allocator);
      let mut visitor =
        AssetImportMetaUrlVisit { path: args.path, ast_builder, plugin_context: ctx };
      visitor.visit_program(fields.program);
    });
    Ok(args.ast)
  }
}

pub struct AssetImportMetaUrlVisit<'ast, 'a> {
  path: &'a Path,
  ast_builder: AstBuilder<'ast>,
  plugin_context: &'a SharedPluginContext,
}

impl<'ast, 'a> VisitMut<'ast> for AssetImportMetaUrlVisit<'ast, 'a> {
  fn visit_new_expression(&mut self, it: &mut oxc::ast::ast::NewExpression<'ast>) {
    if let Some(id) = it.callee.as_identifier() {
      if id.name == "URL"
        && it.arguments.len() == 2
        && it.arguments[1].as_expression().is_some_and(|expr| matches_import_meta_url(expr))
      {
        match it.arguments[0].as_expression() {
          Some(Expression::StringLiteral(lit)) => {
            // TODO: is it okay to do file io during visitor?
            if let Some(asset_path) = self.path.parent().map(|dir| dir.join(lit.value.as_str())) {
              if let Ok(data) = std::fs::read(asset_path) {
                // TODO: it's going to be not possible to synchronously emit_file
                //       if rolldown supports `assetFileNames` js function?
                let reference_id = self.plugin_context.emit_file(EmittedAsset {
                  file_name: None,
                  name: Some(lit.value.as_str().to_string()),
                  source: rolldown_common::AssetSource::Buffer(data),
                });
                *it.arguments.get_mut(0).unwrap() = self.ast_builder.argument_expression(
                  self.ast_builder.expression_member(
                    self.ast_builder.member_expression_static(
                      SPAN,
                      self.ast_builder.expression_meta_property(
                        SPAN,
                        self.ast_builder.identifier_name(SPAN, "import"),
                        self.ast_builder.identifier_name(SPAN, "meta"),
                      ),
                      self
                        .ast_builder
                        .identifier_name(SPAN, format!("ROLLUP_FILE_URL_{reference_id}")),
                      false,
                    ),
                  ),
                );
              }
            }
          }
          _ => {}
        }
      }
    }
  }
}

fn matches_import_meta(expr: &Expression) -> bool {
  matches!(expr, Expression::MetaProperty(e) if e.meta.name == "import" && e.property.name == "meta")
}

fn matches_import_meta_url(expr: &Expression) -> bool {
  expr.as_member_expression().is_some_and(|expr| {
    matches_import_meta(expr.object()) && expr.static_property_name() == Some("url")
  })
}

#[tokio::test(flavor = "multi_thread")]
async fn test_asset_import_meta_url_plugin() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        cwd: Some(cwd),
        ..Default::default()
      },
      vec![Arc::new(AssetImportMetaUrlPlugin {})],
    )
    .await;
}
