use std::sync::Arc;

use rolldown_common::RollupRenderedChunk;

/// Addon hooks: `banner`, `footer`, `intro`, `outro`
#[derive(Debug)]
pub struct HookAddonArgs {
  pub chunk: Arc<RollupRenderedChunk>,
}
