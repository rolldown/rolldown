use std::borrow::Cow;

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::EmittedChunk;
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct EmitChunkPlugin;

impl Plugin for EmitChunkPlugin {
  fn name(&self) -> Cow<'static, str> {
    "emit-chunk-plugin".into()
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> Result<(), anyhow::Error> {
    ctx.emit_chunk(EmittedChunk {
      id: "./other.js".into(),
      name: Some("main".into()),
      ..Default::default()
    })?;
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn user_entry_facade_keeps_name_over_emitted_chunk() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
        chunk_filenames: Some("[name].js".to_string().into()),
        ..Default::default()
      },
      vec![Plugin::new_shared(EmitChunkPlugin)],
    )
    .await;
}
