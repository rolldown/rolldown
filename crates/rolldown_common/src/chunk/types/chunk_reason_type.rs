#[derive(Debug, Default)]
pub enum ChunkReasonType {
  AdvancedChunks {
    group_index: u32,
  },
  PreserveModules,
  Entry,
  #[default]
  Common,
}

impl std::fmt::Display for ChunkReasonType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.as_static_str())
  }
}

impl ChunkReasonType {
  pub fn group_index(&self) -> Option<u32> {
    match self {
      ChunkReasonType::AdvancedChunks { group_index } => Some(*group_index),
      _ => None,
    }
  }

  pub fn as_static_str(&self) -> &'static str {
    match self {
      ChunkReasonType::AdvancedChunks { .. } => "advanced-chunks",
      ChunkReasonType::PreserveModules => "preserve-modules",
      ChunkReasonType::Entry => "entry",
      ChunkReasonType::Common => "common",
    }
  }
}
