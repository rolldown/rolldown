use oxc::span::Atom;
use rolldown_common::SymbolRef;
use rustc_hash::FxHashMap;

use crate::{
  InputOptions,
  {chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput},
};

pub struct ModuleRenderContext<'a> {
  pub input_options: &'a InputOptions,
  pub canonical_names: &'a FxHashMap<SymbolRef, Atom>,
  pub graph: &'a LinkStageOutput,
  pub chunk_graph: &'a ChunkGraph,
}
