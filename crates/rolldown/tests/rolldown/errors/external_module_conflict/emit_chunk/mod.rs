use std::{borrow::Cow, path::Path};

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{EmittedChunk, IsExternal};
use rolldown_plugin::{
  HookTransformArgs, HookTransformReturn, HookUsage, Plugin, SharedTransformPluginContext,
};
use rolldown_std_utils::PathExt as _;
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

/// Emits lib.js as a chunk. `./lib.js` in main.js already made that id external.
#[derive(Debug)]
struct EmitExternalIdAsChunk;

impl Plugin for EmitExternalIdAsChunk {
  fn name(&self) -> Cow<'static, str> {
    "emit-external-id-as-chunk".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    let id = Path::new(args.id);
    if id.ends_with("trigger.js") {
      ctx.emit_chunk(EmittedChunk {
        id: id.with_file_name("lib.js").expect_to_str().to_string(),
        name: Some("lib-entry".into()),
        ..Default::default()
      })?;
    }
    Ok(None)
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn emitted_chunk_of_external_id_is_an_error() {
  manual_integration_test!()
    .build(TestMeta { expect_error: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("main".to_string()),
          import: "./main.js".to_string(),
        }]),
        external: Some(IsExternal::from(vec!["./lib.js".to_string()])),
        ..Default::default()
      },
      vec![Plugin::new_shared(EmitExternalIdAsChunk)],
    )
    .await;
}
