use arcstr::ArcStr;
use itertools::Itertools;
use rolldown_common::ChunkIdx;
use rustc_hash::FxHashMap;

use crate::{
  chunk_graph::ChunkGraph, stages::generate_stage::GenerateStage,
  utils::chunk::deconflict_chunk_symbols::InlineNamingInput,
};

impl GenerateStage<'_> {
  /// What each file that reads a record adds to its own deconflict pass: the modules and
  /// synthetic statements of the records it carries share the file's namespace, and the file
  /// needs one bridge per record it reads, one per record each carried factory reads, and the
  /// factories' `exports` parameter. See internal-docs/inline-common-chunks/implementation.md
  /// ("Deconflicting").
  pub(in crate::stages::generate_stage) fn inline_naming_inputs(
    &self,
    chunk_graph: &ChunkGraph,
  ) -> FxHashMap<ChunkIdx, InlineNamingInput> {
    let modules = &self.link_output.module_table.modules;
    let chunk_name = |chunk_idx: ChunkIdx| -> ArcStr {
      chunk_graph.chunk_table[chunk_idx]
        .name
        .clone()
        .expect("records are named before any file is deconflicted")
    };
    let bridges = |reader: ChunkIdx| {
      self
        .inline_state
        .readers_of(reader)
        .iter()
        .map(|record| (*record, chunk_name(*record)))
        .collect_vec()
    };
    self
      .inline_state
      .reading_files()
      .iter()
      .map(|&file| {
        let carried = self.inline_state.carried_by(file);
        let chunks = std::iter::once(file).chain(carried.iter().copied()).collect_vec();
        // The renamer walks the list in reverse to give later-executing modules naming
        // priority, and requires ascending execution order for that.
        let naming_modules = chunks
          .iter()
          .flat_map(|chunk_idx| chunk_graph.chunk_table[*chunk_idx].modules.iter().copied())
          .sorted_by_key(|module_idx| modules[*module_idx].exec_order())
          .collect_vec();
        let input = InlineNamingInput {
          modules: naming_modules,
          chunks,
          file_bridges: bridges(file),
          factories: carried.iter().map(|record| (*record, bridges(*record))).collect(),
        };
        (file, input)
      })
      .collect()
  }
}
