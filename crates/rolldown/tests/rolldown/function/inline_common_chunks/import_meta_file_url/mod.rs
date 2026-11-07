use std::borrow::Cow;

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, EmittedAsset, InlineCommonChunksOptions, ManualCodeSplittingOptions,
  PreserveEntrySignatures,
};
use rolldown_plugin::{
  HookResolveFileUrlArgs, HookResolveFileUrlReturn, HookTransformOutput, HookUsage, Plugin,
  PluginContext,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

/// Emits an asset for `shared.js`, puts its reference id into the `__REF__` placeholder and, like
/// Vite under a relative `base`, resolves it against the URL of the file that prints it.
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
    if !args.id.ends_with("shared.js") {
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

  async fn resolve_file_url(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveFileUrlArgs<'_>,
  ) -> HookResolveFileUrlReturn {
    // The `#hook` suffix tells this result apart from rolldown's own default rendering.
    Ok(Some(format!("new URL({:?}, import.meta.url).href + '#hook'", args.relative_path)))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform | HookUsage::ResolveFileUrl
  }
}

/// `shared.js` reads `import.meta.ROLLDOWN_FILE_URL_*`, and its two readers are written to
/// different directories. The common chunk is still a record: each carrier's copy is resolved by
/// the `resolveFileUrl` hook against that carrier, so both entries name the same asset.
#[tokio::test(flavor = "multi_thread")]
async fn import_meta_file_url() {
  manual_integration_test!()
    .build(TestMeta { hidden_runtime_module: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![
          InputItem { name: Some("x/a".into()), import: "./a.js".into() },
          InputItem { name: Some("y/z/b".into()), import: "./b.js".into() },
        ]),
        preserve_entry_signatures: Some(PreserveEntrySignatures::False),
        code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
          experimental_inline_common_chunks: Some(InlineCommonChunksOptions {
            max_size: Some(4096.0),
            exclude: None,
          }),
          ..Default::default()
        })),
        ..Default::default()
      },
      vec![Plugin::new_shared(EmitAsset)],
    )
    .await;
}
