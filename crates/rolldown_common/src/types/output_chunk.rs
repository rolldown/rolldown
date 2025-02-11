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
  pub keys: Vec<ModuleId>,
  pub values: Vec<RenderedModule>,
}

impl From<FxHashMap<ModuleId, RenderedModule>> for Modules {
  fn from(value: FxHashMap<ModuleId, RenderedModule>) -> Self {
    let mut kvs = value.into_iter().collect::<Vec<_>>();
    kvs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut keys = Vec::with_capacity(kvs.len());
    let mut values = Vec::with_capacity(kvs.len());
    for (k, v) in kvs {
      keys.push(k);
      values.push(v);
    }
    Self { keys, values }
  }
}
