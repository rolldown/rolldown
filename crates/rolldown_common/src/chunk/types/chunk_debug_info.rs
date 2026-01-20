use std::fmt;

/// Debug information attached to chunks when `experimental.attachDebugInfo: 'full'` is enabled.
#[derive(Debug, Clone)]
pub enum ChunkDebugInfo {
  /// Reason why this chunk was created
  CreateReason(String),
  /// Information about a facade chunk that was eliminated and merged into this chunk
  EliminatedFacadeChunk {
    /// Name of the eliminated chunk
    chunk_name: String,
    /// Module ID of the entry module from the eliminated chunk
    entry_module_id: String,
  },
}

impl fmt::Display for ChunkDebugInfo {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ChunkDebugInfo::CreateReason(reason) => write!(f, "{reason}"),
      ChunkDebugInfo::EliminatedFacadeChunk { chunk_name, entry_module_id } => {
        write!(
          f,
          "Eliminated Facade Chunk: [Chunk-Name: {chunk_name}] [Entry-Module-Id: {entry_module_id}]"
        )
      }
    }
  }
}
