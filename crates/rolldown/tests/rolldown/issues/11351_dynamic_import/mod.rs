use std::borrow::Cow;

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::EmittedChunk;
use rolldown_plugin::{HookBuildStartArgs, HookUsage, Plugin, PluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct EmitOther;

impl Plugin for EmitOther {
  fn name(&self) -> Cow<'static, str> {
    "emit-other".into()
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &HookBuildStartArgs<'_>,
  ) -> Result<(), anyhow::Error> {
    ctx.emit_chunk(EmittedChunk { id: "./other.js".to_string(), ..Default::default() })?;
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }
}

// Only `other.js` loads `lazy.js`. So `lazy.js` is a late entry, and the chunk of `other.js` has
// the covering bits of the runtime. As the host, that chunk must export the helpers to `lazy.js`,
// and these exports change the signature of `other.js`. That signature is fixed, because `other.js`
// has exports and the default `exports-only` applies. An emitted entry must keep its signature, as
// an `input` entry does, so the runtime goes into its own chunk. Before the fix, the chunk of
// `other.js` became the host, and the emitted entry became a facade.
// See internal-docs/code-splitting/implementation.md#runtime-module-placement
// See https://github.com/rolldown/rolldown/issues/11351
#[tokio::test(flavor = "multi_thread")]
async fn emitted_entry_with_strict_signature_does_not_host_runtime() {
  manual_integration_test!()
    .build(TestMeta::default())
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
        ..Default::default()
      },
      vec![Plugin::new_shared(EmitOther)],
    )
    .await;
}
