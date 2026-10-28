use std::borrow::Cow;

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::EmittedChunk;
use rolldown_plugin::{HookBuildStartArgs, HookUsage, Plugin, PluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

/// This plugin emits `other.js` as an entry. `rolldown-plugin-dts` emits its declaration entries in
/// the same way.
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

// `dep.js` needs `__exportAll` only after facade elimination, for the namespace object of
// `import('./dep.js')`. The entries that load `dep.js` never load `other.js`. So the chunk of the
// emitted entry `other.js` must not be the host of the runtime. The runtime goes into its own
// chunk, as when `other.js` is an `input` entry.
// See https://github.com/rolldown/rolldown/issues/11351
#[tokio::test(flavor = "multi_thread")]
async fn emitted_entry_does_not_host_runtime_for_late_consumer() {
  manual_integration_test!()
    .build(TestMeta::default())
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![
          InputItem { name: Some("index".into()), import: "./index.js".into() },
          InputItem { name: Some("secondary".into()), import: "./secondary.js".into() },
        ]),
        ..Default::default()
      },
      vec![Plugin::new_shared(EmitOther)],
    )
    .await;
}
