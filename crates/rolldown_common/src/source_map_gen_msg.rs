use std::sync::Arc;

use crate::PendingSourcemap;

#[derive(Debug)]
pub enum SourceMapGenMsg {
  /// Ask the sourcemap worker to generate the map. The plugin driver sends it
  /// once the module's transform hooks are done. See `PendingSourcemap`.
  MagicString(Arc<PendingSourcemap>),
  Terminate,
}
