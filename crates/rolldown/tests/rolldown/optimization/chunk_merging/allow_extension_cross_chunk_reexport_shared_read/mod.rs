use std::borrow::Cow;

use rolldown::{BundlerOptions, InputItem, OutputFormat, PreserveEntrySignatures};
use rolldown_common::{
  CodeSplittingMode, EmittedChunk, ManualCodeSplittingOptions, MatchGroup, MatchGroupName,
  MatchGroupTest,
};
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_testing::{
  integration_test::NamedBundlerOptions, manual_integration_test, test_config::TestMeta,
};
use rolldown_utils::js_regex::HybridRegex;

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
      id: "./seg.js".to_string(),
      preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
      ..Default::default()
    })?;
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read() {
  // `seg.js` is an emitted AllowExtension entry, and the `group` chunk holds its module,
  // `barrel.js` and the CommonJS modules. The entry re-exports `foreign` from the `dep.js` chunk,
  // so it keeps its facade (issue #11767). The facade and the `app.js` entry re-export the same
  // properties of CommonJS modules. Node reads each property one time, when it imports the
  // CommonJS module. So both entries call one accessor in the `group` chunk, and the first call
  // reads the property:
  // - The getter of `n` counts its reads. Both entries get `1`, in each load order.
  // - The getter of `x` throws. Both entries get `undefined`, and the getter runs one time.
  // - `mut.cjs` writes `m`. After the first read, both entries keep the value of that read. In CJS
  //   output, the first read runs at the first access of an export, so it can see a write that
  //   Node does not see. Node reads `m` when it imports `mut.cjs`.
  // - `barrel.js` gets `y` through `export *` from two CommonJS modules, so the output cannot tell
  //   which module gives `y`. Both entries read `y` from the namespace of `barrel.js`, and they
  //   share the read of `y` from that namespace.
  // `other.js` loads the `group` chunk and reads no property, so the getter of `n` does not run.
  let options = BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("app".into()), import: "./app.js".into() },
      InputItem { name: Some("other".into()), import: "./other.js".into() },
      InputItem { name: Some("dep".into()), import: "./dep.js".into() },
    ]),
    code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
      include_dependencies_recursively: Some(false),
      groups: Some(vec![MatchGroup {
        name: MatchGroupName::Static("group".to_string()),
        test: Some(MatchGroupTest::Regex(
          HybridRegex::new(r"(seg\.js|barrel\.js|\.cjs)$").unwrap(),
        )),
        ..Default::default()
      }]),
      ..Default::default()
    })),
    ..Default::default()
  };
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, ..Default::default() })
    .run_multiple(
      vec![
        NamedBundlerOptions {
          description: None,
          options: options.clone(),
          snapshot: None,
          config_name: None,
          expect_execution_failure: None,
        },
        // In CJS output, the exports of `app.js` are getters that call the accessor.
        NamedBundlerOptions {
          description: Some("cjs".to_string()),
          options: BundlerOptions { format: Some(OutputFormat::Cjs), ..options },
          snapshot: None,
          config_name: Some("cjs".to_string()),
          expect_execution_failure: None,
        },
      ],
      vec![Plugin::new_shared(EmitChunkPlugin)],
    )
    .await;
}
