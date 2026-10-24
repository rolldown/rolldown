use std::borrow::Cow;

use rolldown::{BundlerOptions, InputItem, OutputFormat};
use rolldown_common::{ExperimentalOptions, RUNTIME_MODULE_KEY};
use rolldown_plugin::{HookTransformReturn, HookUsage, Plugin, SharedTransformPluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct InstrumentRuntimeHelperPlugin;

impl Plugin for InstrumentRuntimeHelperPlugin {
  fn name(&self) -> Cow<'static, str> {
    "instrument-runtime-helper".into()
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if args.id != RUNTIME_MODULE_KEY {
      return Ok(None);
    }

    let declaration = "export var __esmMin = (fn, res, err) => () => {";
    assert!(args.code.contains(declaration), "the plugin must instrument __esmMin");
    let code = format!(
      "import {{ ok }} from 'node:assert/strict';\n{}",
      args.code.replace(declaration, &format!("{declaration}\n  ok(true);"))
    );
    Ok(Some(rolldown_plugin::HookTransformOutput { code: Some(code), ..Default::default() }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn order_lowering_keeps_transformed_runtime_external_import() {
  // Order lowering retains __esmMin after tree shaking. Its external dependency must
  // survive too, even when the import statement's inclusion bit is unset.
  // See internal-docs/runtime-helpers/implementation.md.
  manual_integration_test!()
    .build(TestMeta { snapshot: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("main".to_string()),
          import: "./main.js".to_string(),
        }]),
        external: Some(vec!["node:assert/strict".to_string()].into()),
        format: Some(OutputFormat::Esm),
        strict_execution_order: Some(true),
        experimental: Some(ExperimentalOptions {
          on_demand_wrapping: Some(false),
          ..Default::default()
        }),
        ..Default::default()
      },
      vec![Plugin::new_shared(InstrumentRuntimeHelperPlugin)],
    )
    .await;
}
