use std::fmt;

/// The specific optimization scenario that led to a facade chunk being eliminated.
#[derive(Debug, Clone, Copy)]
pub enum FacadeChunkEliminationReason {
  /// Emitted chunk (AllowExtension) merged into manual code splitting group.
  /// Export name conflicts are checked before merging.
  EmittedChunkMergedIntoManualGroup,
  /// Dynamic entry chunk merged into manual code splitting group.
  DynamicEntryMergedIntoManualGroup,
  /// Dynamic entry chunk merged into user-defined entry chunk.
  DynamicEntryMergedIntoUserDefinedEntry,
  /// Dynamic entry chunk merged into common chunk.
  DynamicEntryMergedIntoCommonChunk,
}

impl fmt::Display for FacadeChunkEliminationReason {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      FacadeChunkEliminationReason::EmittedChunkMergedIntoManualGroup => {
        write!(f, "Emitted chunk (AllowExtension) merged into manual code splitting group")
      }
      FacadeChunkEliminationReason::DynamicEntryMergedIntoManualGroup => {
        write!(f, "Dynamic entry chunk merged into manual code splitting group")
      }
      FacadeChunkEliminationReason::DynamicEntryMergedIntoUserDefinedEntry => {
        write!(f, "Dynamic entry chunk merged into user-defined entry chunk")
      }
      FacadeChunkEliminationReason::DynamicEntryMergedIntoCommonChunk => {
        write!(f, "Dynamic entry chunk merged into common chunk")
      }
    }
  }
}

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
    /// The specific optimization scenario that led to this elimination
    reason: FacadeChunkEliminationReason,
  },
}

impl fmt::Display for ChunkDebugInfo {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ChunkDebugInfo::CreateReason(reason) => write!(f, "{reason}"),
      ChunkDebugInfo::EliminatedFacadeChunk { chunk_name, entry_module_id, reason } => {
        write!(
          f,
          "Eliminated Facade Chunk: [Chunk-Name: {chunk_name}] [Entry-Module-Id: {entry_module_id}] [Reason: {reason}]"
        )
      }
    }
  }
}
