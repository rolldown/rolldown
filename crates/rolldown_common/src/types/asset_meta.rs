use crate::RenderedChunk;

pub enum AssetMeta {
  Ecma(RenderedChunk),
  // Using Variant `None` instead of `Option<AssetMeta>` to make it friendly to use pattern matching.
  None,
}
