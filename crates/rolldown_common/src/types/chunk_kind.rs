use crate::NormalModuleId;

#[derive(Debug)]
pub enum ChunkKind {
  EntryPoint { is_user_defined: bool, bit: u32, module: NormalModuleId },
  Common,
}

impl Default for ChunkKind {
  fn default() -> Self {
    Self::Common
  }
}
