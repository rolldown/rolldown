use std::borrow::Cow;

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::EmittedAsset;
use rolldown_plugin::{HookTransformOutput, HookUsage, Plugin};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

/// Emits an asset for `main.js` and puts its reference id into the `__REF__` placeholder.
#[derive(Debug)]
struct EmitAsset;

impl Plugin for EmitAsset {
  fn name(&self) -> Cow<'static, str> {
    "emit-asset".into()
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !args.id.ends_with("main.js") {
      return Ok(None);
    }
    let reference_id = ctx.emit_file(
      EmittedAsset {
        name: Some("asset.txt".into()),
        original_file_name: None,
        file_name: None,
        source: "asset".to_string().into(),
      },
      None,
      None,
    )?;
    Ok(Some(HookTransformOutput {
      code: Some(args.code.replace("__REF__", &reference_id)),
      ..Default::default()
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn file_url_shadowed_by_binding() {
  manual_integration_test!()
    .build(TestMeta::default())
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
        ..Default::default()
      },
      vec![Plugin::new_shared(EmitAsset)],
    )
    .await;
}
