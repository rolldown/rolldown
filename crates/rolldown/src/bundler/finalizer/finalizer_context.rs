use oxc::span::Atom;
use rolldown_common::{ModuleId, SymbolRef};

use rustc_hash::FxHashMap;

use crate::bundler::{
  chunk_graph::ChunkGraph,
  module::{ModuleVec, NormalModule},
  runtime::RuntimeModuleBrief,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
  utils::symbols::Symbols,
};

pub struct FinalizerContext<'me> {
  pub id: ModuleId,
  pub module: &'me NormalModule,
  pub modules: &'me ModuleVec,
  pub linking_info: &'me LinkingMetadata,
  pub linking_infos: &'me LinkingMetadataVec,
  pub symbols: &'me Symbols,
  pub canonical_names: &'me FxHashMap<SymbolRef, Atom>,
  pub runtime: &'me RuntimeModuleBrief,
  pub chunk_graph: &'me ChunkGraph,
}
