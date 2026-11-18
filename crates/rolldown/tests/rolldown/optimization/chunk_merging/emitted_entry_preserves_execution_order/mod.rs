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
    ctx.emit_chunk(EmittedChunk {
      id: "./base.js".into(),
      name: Some("base".into()),
      ..Default::default()
    })?;
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn emitted_entry_preserves_execution_order() {
  manual_integration_test!()
    .build(TestMeta::default())
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
        strict_execution_order: Some(true),
        ..Default::default()
      },
      vec![Plugin::new_shared(EmitEntriesPlugin)],
    )
    .await;
}
