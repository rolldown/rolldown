use crate::bundler::chunk::render_chunk::RenderedChunk;
use rolldown_common::ImportKind;

#[derive(Debug)]
pub struct HookResolveIdArgs<'a> {
  pub importer: Option<&'a str>,
  pub source: &'a str,
  pub options: HookResolveIdArgsOptions,
}

#[derive(Debug, Clone)]
pub struct HookResolveIdArgsOptions {
  pub is_entry: bool,
  // Rollup hasn't this filed, but since Rolldown support cjs as first citizen, so we need to generate `kind` to distinguish it.
  pub kind: ImportKind,
}

#[derive(Debug)]
pub struct HookTransformArgs<'a> {
  pub id: &'a str,
  pub code: &'a String,
}

#[derive(Debug)]
pub struct HookLoadArgs<'a> {
  pub id: &'a str,
}

#[derive(Debug, Default)]
pub struct HookBuildEndArgs {
  pub error: String,
}

#[derive(Debug)]
pub struct RenderChunkArgs<'a> {
  pub code: String,
  pub chunk: &'a RenderedChunk,
}
