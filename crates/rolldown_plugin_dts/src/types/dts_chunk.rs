use arcstr::ArcStr;
use rolldown_common::ModuleIdx;

pub struct DtsChunk {
  pub dts_modules: Vec<ModuleIdx>,
  pub name: ArcStr,
  pub is_entry: bool,
}
