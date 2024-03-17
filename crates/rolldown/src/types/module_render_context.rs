use rolldown_common::SymbolRef;
use rolldown_rstr::Rstr;
use rustc_hash::FxHashMap;

use crate::{
  chunk_graph::ChunkGraph, options::normalized_input_options::NormalizedInputOptions,
  stages::link_stage::LinkStageOutput,
};

pub struct ModuleRenderContext<'a> {
  pub input_options: &'a NormalizedInputOptions,
  pub canonical_names: &'a FxHashMap<SymbolRef, Rstr>,
  pub graph: &'a LinkStageOutput,
  pub chunk_graph: &'a ChunkGraph,
}
