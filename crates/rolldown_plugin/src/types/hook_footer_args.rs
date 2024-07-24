use rolldown_common::RollupRenderedChunk;

#[derive(Debug)]
pub struct HookFooterArgs<'a> {
  pub chunk: &'a RollupRenderedChunk,
}
