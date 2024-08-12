use rolldown_common::RollupRenderedChunk;

/// Addon hooks: `banner`, `footer`, `intro`, `outro`
#[derive(Debug)]
pub struct HookAddonArgs<'a> {
  pub chunk: &'a RollupRenderedChunk,
}
