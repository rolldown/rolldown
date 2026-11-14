use std::{
  borrow::Cow,
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
};

use rolldown::{Bundler, BundlerOptions, InputItem, PreserveEntrySignatures};
use rolldown_common::{
  CodeSplittingMode, InlineCommonChunksOptions, ManualCodeSplittingOptions, Output,
};
use rolldown_plugin::{
  HookRenderChunkArgs, HookRenderChunkReturn, HookUsage, Plugin, PluginContext,
};

#[derive(Debug)]
struct InspectOrigin {
  inline: bool,
  calls: Arc<AtomicUsize>,
}

impl Plugin for InspectOrigin {
  fn name(&self) -> Cow<'static, str> {
    "inspect-origin".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::RenderChunk
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    assert!(Arc::ptr_eq(&args.chunk, &args.chunks[&args.chunk.filename]));
    for chunk in args.chunks.values() {
      let logical = &args.chunk_table[chunk.originate_from];
      let ids: Vec<_> = logical.modules.iter().map(|&idx| args.module_table[idx].id()).collect();
      for id in &ids {
        assert!(chunk.module_ids.contains(id));
      }
      if chunk.is_entry {
        assert_eq!(ids.len(), 1);
        assert!(Path::new(ids[0].as_str()).ends_with(format!("{}.js", chunk.name)));
        assert_eq!(
          chunk.module_ids.iter().any(|id| Path::new(id.as_str()).ends_with("shared.js")),
          self.inline
        );
        assert_eq!(
          chunk.modules.keys.iter().any(|id| Path::new(id.as_str()).ends_with("shared.js")),
          self.inline
        );
      }
    }
    self.calls.fetch_add(1, Ordering::SeqCst);
    Ok(None)
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn render_chunk_origin_resolves_logical_members() {
  for inline in [false, true] {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut bundler = Bundler::with_plugins(
      BundlerOptions {
        input: Some(
          ["a", "b"]
            .map(|name| InputItem { name: Some(name.into()), import: format!("./{name}.js") })
            .into(),
        ),
        cwd: Some(
          concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/plugin/render_chunk_origin").into(),
        ),
        preserve_entry_signatures: Some(PreserveEntrySignatures::False),
        strict_execution_order: Some(true),
        code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
          experimental_inline_common_chunks: inline
            .then_some(InlineCommonChunksOptions { max_size: Some(f64::INFINITY), exclude: None }),
          ..Default::default()
        })),
        ..Default::default()
      },
      vec![Plugin::new_shared(InspectOrigin { inline, calls: Arc::clone(&calls) })],
    )
    .unwrap();
    let output = bundler.generate().await.unwrap();
    assert_eq!(
      calls.load(Ordering::SeqCst),
      output.assets.iter().filter(|asset| matches!(asset, Output::Chunk(_))).count()
    );
  }
}
