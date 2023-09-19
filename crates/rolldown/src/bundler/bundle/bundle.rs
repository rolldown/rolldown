use anyhow::Ok;

use super::asset::Asset;
use crate::bundler::{
  chunk::{chunk::Chunk, ChunksVec},
  graph::graph::Graph,
  module::module::ModuleFinalizeContext,
  options::{
    normalized_input_options::NormalizedInputOptions,
    normalized_output_options::NormalizedOutputOptions,
  },
};

pub struct Bundle<'a> {
  graph: &'a mut Graph,
  output_options: &'a NormalizedOutputOptions,
}

impl<'a> Bundle<'a> {
  pub fn new(graph: &'a mut Graph, output_options: &'a NormalizedOutputOptions) -> Self {
    Self {
      graph,
      output_options,
    }
  }

  pub fn generate_chunks(graph: &Graph) -> ChunksVec {
    let mut chunks = ChunksVec::with_capacity(graph.entries.len());
    let mut modules = graph.modules.iter().map(|m| m.id()).collect::<Vec<_>>();
    modules.sort_by_key(|id| graph.modules[*id].exec_order());
    let chunk = Chunk::new(Some("main".to_string()), true, modules);
    chunks.push(chunk);
    chunks
  }

  pub fn generate(
    &mut self,
    input_options: &'a NormalizedInputOptions,
  ) -> anyhow::Result<Vec<Asset>> {
    use rayon::prelude::*;
    let mut chunks = Self::generate_chunks(self.graph);

    chunks
      .iter_mut()
      .par_bridge()
      .for_each(|chunk| chunk.render_file_name(self.output_options));

    chunks.iter_mut().par_bridge().for_each(|chunk| {
      chunk.de_conflict(self.graph);
    });

    chunks.iter_mut().for_each(|chunk| {
      if chunk.is_entry {
        chunk.initialize_exports(&mut self.graph.modules, &self.graph.symbols);
      }
    });

    self
      .graph
      .modules
      .iter_mut()
      .par_bridge()
      .for_each(|module| {
        module.finalize(ModuleFinalizeContext {
          canonical_names: &chunks[0].canonical_names,
          symbols: &self.graph.symbols,
        });
      });

    let assets = chunks
      .iter()
      .map(|c| {
        let content = c.render(self.graph, input_options).unwrap();

        Asset {
          file_name: c.file_name.clone().unwrap(),
          content,
        }
      })
      .collect::<Vec<_>>();

    Ok(assets)
  }
}
