use oxc::span::CompactStr;
use rolldown_common::{
  ExternalModuleVec, NormalModule, NormalModuleId, NormalModuleVec, SymbolRef,
};

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
  pub id: NormalModuleId,
  pub module: &'me NormalModule,
  pub modules: &'me NormalModuleVec,
  pub external_modules: &'me ExternalModuleVec,
  pub linking_info: &'me LinkingMetadata,
  pub linking_infos: &'me LinkingMetadataVec,
  pub symbols: &'me Symbols,
  pub canonical_names: &'me FxHashMap<SymbolRef, Rstr>,
  pub runtime: &'me RuntimeModuleBrief,
  pub chunk_graph: &'me ChunkGraph,
  pub options: &'me SharedOptions,
  pub top_level_cache: &'me FxHashMap<SymbolRef, FxHashMap<Box<[CompactStr]>, (SymbolRef, usize)>>,
}
