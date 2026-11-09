use crate::{HookLoadArgs, HookRenderChunkArgs, HookResolveIdArgs, HookTransformArgs};

/// The hook call that [`crate::Plugin::hook_filter_matches`] checks.
#[derive(Clone, Copy)]
pub enum HookFilterArgs<'a> {
  ResolveId(&'a HookResolveIdArgs<'a>),
  Load(&'a HookLoadArgs<'a>),
  Transform(&'a HookTransformArgs<'a>),
  RenderChunk(&'a HookRenderChunkArgs<'a>),
}
