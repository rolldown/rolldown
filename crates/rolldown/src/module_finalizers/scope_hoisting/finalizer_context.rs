use rolldown_common::{EcmaModule, IndexModules, ModuleIdx, SymbolRef};

use rolldown_rstr::Rstr;
use rustc_hash::FxHashMap;

use crate::{
  chunk_graph::ChunkGraph,
  runtime::RuntimeModuleBrief,
  types::{
    linking_metadata::{LinkingMetadata, LinkingMetadataVec},
    symbols::Symbols,
  },
  SharedOptions,
};

pub struct ScopeHoistingFinalizerContext<'me> {
  pub id: ModuleIdx,
  pub module: &'me EcmaModule,
  pub modules: &'me IndexModules,
  pub linking_info: &'me LinkingMetadata,
  pub linking_infos: &'me LinkingMetadataVec,
  pub symbols: &'me Symbols,
  pub canonical_names: &'me FxHashMap<SymbolRef, Rstr>,
  pub runtime: &'me RuntimeModuleBrief,
  pub chunk_graph: &'me ChunkGraph,
  pub options: &'me SharedOptions,
}
