use oxc::span::Atom;
use rolldown_common::{NormalModule, NormalModuleId, SymbolRef};

use rustc_hash::FxHashMap;

use crate::bundler::{
  chunk_graph::ChunkGraph,
  runtime::RuntimeModuleBrief,
  types::{
    linking_metadata::{LinkingMetadata, LinkingMetadataVec},
    module_table::NormalModuleVec,
    symbols::Symbols,
  },
};

pub struct FinalizerContext<'me> {
  pub id: NormalModuleId,
  pub module: &'me NormalModule,
  pub modules: &'me NormalModuleVec,
  pub linking_info: &'me LinkingMetadata,
  pub linking_infos: &'me LinkingMetadataVec,
  pub symbols: &'me Symbols,
  pub canonical_names: &'me FxHashMap<SymbolRef, Atom>,
  pub runtime: &'me RuntimeModuleBrief,
  pub chunk_graph: &'me ChunkGraph,
}
