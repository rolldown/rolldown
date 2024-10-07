use rolldown_common::{IndexModules, ModuleIdx, NormalModule, SymbolRef, SymbolRefDb};

use rolldown_rstr::Rstr;
use rustc_hash::FxHashMap;

use crate::{
  chunk_graph::ChunkGraph,
  runtime::RuntimeModuleBrief,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
  SharedOptions,
};

pub struct ScopeHoistingFinalizerContext<'me> {
  pub id: ModuleIdx,
  pub module: &'me NormalModule,
  pub modules: &'me IndexModules,
  pub linking_info: &'me LinkingMetadata,
  pub linking_infos: &'me LinkingMetadataVec,
  pub symbol_db: &'me SymbolRefDb,
  pub canonical_names: &'me FxHashMap<SymbolRef, Rstr>,
  pub runtime: &'me RuntimeModuleBrief,
  pub chunk_graph: &'me ChunkGraph,
  pub options: &'me SharedOptions,
}
