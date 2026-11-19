use std::borrow::Cow;

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::EmittedChunk;
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct EmitEntriesPlugin;

impl Plugin for EmitEntriesPlugin {
  fn name(&self) -> Cow<'static, str> {
    "emit-entries-plugin".into()
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> Result<(), anyhow::Error> {
    for (name, id) in [("common", "./one.js"), ("other", "./two.js")] {
      ctx.emit_chunk(EmittedChunk {
        id: id.into(),
        name: Some(name.into()),
        ..Default::default()
      })?;
    }
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn emitted_entry_keeps_name_over_common_chunk() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("noop".into()), import: "./noop.js".into() }]),
        ..Default::default()
      },
      vec![Plugin::new_shared(EmitEntriesPlugin)],
    )
    .await;
}
