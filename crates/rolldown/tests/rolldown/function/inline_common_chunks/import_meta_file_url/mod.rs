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

/// Emits two assets for `shared.js` and puts their reference ids into its `__HOOKED__` and
/// `__PLAIN__` placeholders. `resolveFileUrl` resolves `hooked`, like Vite under a relative `base`,
/// against the URL of the file that prints it, and leaves `plain` to the default rendering.
#[derive(Debug)]
struct EmitAssets;

impl Plugin for EmitAssets {
  fn name(&self) -> Cow<'static, str> {
    "emit-assets".into()
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !args.id.ends_with("shared.js") {
      return Ok(None);
    }
    let mut code = args.code.to_string();
    for name in ["hooked", "plain"] {
      let reference_id = ctx.emit_file(
        EmittedAsset {
          name: Some(format!("{name}.txt")),
          original_file_name: None,
          file_name: None,
          source: name.to_string().into(),
        },
        None,
        None,
      )?;
      code = code.replace(&format!("__{}__", name.to_uppercase()), &reference_id);
    }
    Ok(Some(HookTransformOutput { code: Some(code), ..Default::default() }))
  }

  async fn resolve_file_url(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveFileUrlArgs<'_>,
  ) -> HookResolveFileUrlReturn {
    if args.file_name.starts_with("assets/plain-") {
      return Ok(None);
    }
    // The `#<chunkId>` suffix tells this result apart from the default rendering and names the
    // file it was resolved for.
    Ok(Some(format!(
      "new URL({:?}, import.meta.url).href + {:?}",
      args.relative_path,
      format!("#{}", args.chunk_id)
    )))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform | HookUsage::ResolveFileUrl
  }
}

/// `shared.js` reads `import.meta.ROLLDOWN_FILE_URL_*`, and its two readers are written to
/// different directories. The common chunk becomes a record, and each carrier's copy resolves the
/// references against that carrier, so both entries name the same assets.
#[tokio::test(flavor = "multi_thread")]
async fn import_meta_file_url() {
  manual_integration_test!()
    .build(TestMeta::default())
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
      vec![Plugin::new_shared(EmitAssets)],
    )
    .await;
}
