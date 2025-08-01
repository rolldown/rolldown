use oxc::span::CompactStr;

use crate::ModuleId;

use super::output_chunk::Modules;

// The prefix `Rollup` shows that this is struct is designed for compatibility with Rollup. Adding the `Rollup` prefix to show how much types are only used for compatibility with Rollup.
#[derive(Debug)]
pub struct RollupRenderedChunk {
  // PreRenderedChunk
  pub name: CompactStr,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<ModuleId>,
  pub module_ids: Vec<ModuleId>,
  pub exports: Vec<CompactStr>,
  // RenderedChunk
  pub filename: CompactStr,
  pub modules: Modules,
  pub imports: Vec<CompactStr>,
  pub dynamic_imports: Vec<CompactStr>,
}
