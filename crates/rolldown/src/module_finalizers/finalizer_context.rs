use rolldown_common::{
  ChunkIdx, ConstExportMeta, IndexModules, ModuleIdx, NormalModule, RuntimeModuleBrief,
  SharedFileEmitter, SymbolRef, SymbolRefDb,
};

use rolldown_rstr::Rstr;
use rustc_hash::FxHashMap;

use crate::{
  SharedOptions,
  chunk_graph::ChunkGraph,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

pub struct ScopeHoistingFinalizerContext<'me> {
  pub id: ModuleIdx,
  pub chunk_id: ChunkIdx,
  pub module: &'me NormalModule,
  pub modules: &'me IndexModules,
  pub linking_info: &'me LinkingMetadata,
  pub linking_infos: &'me LinkingMetadataVec,
  pub symbol_db: &'me SymbolRefDb,
  pub canonical_names: &'me FxHashMap<SymbolRef, Rstr>,
  pub runtime: &'me RuntimeModuleBrief,
  pub chunk_graph: &'me ChunkGraph,
  pub options: &'me SharedOptions,
  pub cur_stmt_index: usize,
  pub keep_name_statement_to_insert: Vec<(usize, Rstr, Rstr)>,
  pub file_emitter: &'me SharedFileEmitter,
  pub constant_value_map: &'me FxHashMap<SymbolRef, ConstExportMeta>,
  pub needs_hosted_top_level_binding: bool,
  pub module_namespace_included: bool,
}
