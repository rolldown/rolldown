//! PoC (`poc/chunk-ast-minify`): code that oxc's DCE moves across modules (single-use inlining)
//! out of modules whose text came from a plugin `transform` with a sourcemap. In the chunk AST
//! DCE `global` mode, the moved code must map through the *source* module's `sourcemap_chain`.
//! Run with `ROLLDOWN_CHUNK_AST_DCE=global` and compare the visualized sourcemap with the default.
use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem, SourceMapType};
use rolldown_plugin::{
  HookTransformArgs, HookTransformOutput, HookTransformReturn, HookUsage, Plugin,
  SharedTransformPluginContext,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use string_wizard::{MagicString, SourceMapOptions};

/// Prepends two comment lines to every fixture module, with an exact sourcemap.
#[derive(Debug)]
struct ShiftLinesPlugin;

impl Plugin for ShiftLinesPlugin {
  fn name(&self) -> Cow<'static, str> {
    "shift-lines".into()
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if !args.id.ends_with(".js") || args.id.contains("rolldown") {
      return Ok(None);
    }
    let mut magic_string = MagicString::new(args.code.as_str());
    magic_string.prepend("// shifted by a plugin\n// shifted by a plugin\n");
    Ok(Some(HookTransformOutput {
      code: Some(magic_string.to_string()),
      map: magic_string
        .source_map(SourceMapOptions {
          hires: string_wizard::Hires::True,
          include_content: true,
          source: Arc::from(args.id),
        })
        .into(),
      ..Default::default()
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn plugin_sourcemap_chain() {
  manual_integration_test!()
    .build(TestMeta { visualize_sourcemap: true, expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        sourcemap: Some(SourceMapType::File),
        ..Default::default()
      },
      vec![Plugin::new_shared(ShiftLinesPlugin)],
    )
    .await;
}
