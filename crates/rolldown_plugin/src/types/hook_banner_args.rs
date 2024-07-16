use rolldown_common::RollupRenderedChunk;

#[derive(Debug)]
pub struct HookBannerArgs<'a> {
  pub chunk: &'a RollupRenderedChunk,
}
