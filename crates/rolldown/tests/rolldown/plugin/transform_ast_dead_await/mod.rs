use std::borrow::Cow;

use oxc::{parser::Parser, span::SourceType};
use rolldown::{BundlerOptions, InputItem, RawMinifyOptions};
use rolldown_plugin::{
  HookTransformAstArgs, HookTransformAstReturn, HookUsage, Plugin, PluginContext,
};
use rolldown_testing::{
  integration_test::NamedBundlerOptions, manual_integration_test, test_config::TestMeta,
};

/// Appends `if (false) await 0;` to `pause.js`, whose source text is only `export {};`. Node
/// treats a module with a top-level `await` as async, also when the `await` never runs, so `a.js`
/// waits for `pause.js` and `b.js` initializes `f` first. The output must keep the result of
/// `main`: no early call of `f`. See internal-docs/linking/module-execution-order/implementation.md.
#[derive(Debug)]
struct AppendDeadAwait;

impl Plugin for AppendDeadAwait {
  fn name(&self) -> Cow<'static, str> {
    "append-dead-await".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::TransformAst
  }

  async fn transform_ast(
    &self,
    _ctx: &PluginContext,
    mut args: HookTransformAstArgs<'_>,
  ) -> HookTransformAstReturn {
    if args.id.ends_with("pause.js") {
      args.ast.program.with_mut(|fields| {
        let inserted =
          Parser::new(fields.allocator, "if (false) await 0;", SourceType::mjs()).parse();
        fields.program.body.extend(inserted.program.body);
      });
    }
    Ok(args.ast)
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn transform_ast_dead_await() {
  let options = BundlerOptions {
    input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
    ..Default::default()
  };
  let minify_disabled =
    BundlerOptions { minify: Some(RawMinifyOptions::Bool(false)), ..options.clone() };
  manual_integration_test!()
    .build(TestMeta::default())
    .run_multiple(
      vec![
        NamedBundlerOptions {
          options,
          description: None,
          snapshot: None,
          config_name: None,
          expect_execution_failure: None,
        },
        NamedBundlerOptions {
          options: minify_disabled,
          description: Some("minify: false".into()),
          snapshot: None,
          config_name: None,
          expect_execution_failure: None,
        },
      ],
      vec![Plugin::new_shared(AppendDeadAwait)],
    )
    .await;
}
