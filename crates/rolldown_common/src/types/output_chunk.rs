use arcstr::ArcStr;
use rolldown_rstr::Rstr;
use rolldown_sourcemap::SourceMap;
use rustc_hash::FxHashMap;

use crate::ModuleId;

use super::rendered_module::RenderedModule;

#[derive(Debug, Clone)]
pub struct OutputChunk {
  // PreRenderedChunk
  pub name: ArcStr,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<ModuleId>,
  pub module_ids: Vec<ModuleId>,
  pub exports: Vec<Rstr>,
  // RenderedChunk
  pub filename: ArcStr,
  pub modules: Modules,
  pub imports: Vec<ArcStr>,
  pub dynamic_imports: Vec<ArcStr>,
  // OutputChunk
  pub code: String,
  pub map: Option<SourceMap>,
  pub sourcemap_filename: Option<String>,
  pub preliminary_filename: String,
}

#[derive(Debug, Clone)]
pub struct Modules {
  pub key_to_index: FxHashMap<ModuleId, usize>,
  pub value: Vec<RenderedModule>,
}

impl From<FxHashMap<ModuleId, RenderedModule>> for Modules {
  fn from(value: FxHashMap<ModuleId, RenderedModule>) -> Self {
    let mut key_to_index = FxHashMap::default();
    let value = value
      .into_iter()
      .enumerate()
      .map(|(index, (key, value))| {
        key_to_index.insert(key, index);
        value
      })
      .collect();
    Self { key_to_index, value }
  }
}
