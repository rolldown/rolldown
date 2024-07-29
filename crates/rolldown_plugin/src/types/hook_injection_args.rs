use rolldown_common::RollupRenderedChunk;

#[derive(Debug)]
pub struct HookInjectionArgs<'a> {
  pub chunk: &'a RollupRenderedChunk,
}
