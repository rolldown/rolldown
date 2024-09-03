use oxc::codegen::CodegenReturn;
use rolldown_common::{Chunk, ChunkIdx, InstantiatedChunk, NormalizedBundlerOptions};
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_plugin::SharedPluginDriver;

use crate::{chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput};

pub struct GenerateContext<'a> {
  pub chunk_idx: ChunkIdx,
  pub chunk: &'a Chunk,
  pub options: &'a NormalizedBundlerOptions,
  pub link_output: &'a LinkStageOutput,
  pub chunk_graph: &'a ChunkGraph,
  pub plugin_driver: &'a SharedPluginDriver,
  pub warnings: Vec<BuildDiagnostic>,
  pub module_id_to_codegen_ret: Vec<Option<CodegenReturn>>,
}

pub struct GenerateOutput {
  pub chunks: Vec<InstantiatedChunk>,
  pub warnings: Vec<BuildDiagnostic>,
}

pub trait Generator {
  async fn instantiate_chunk(
    ctx: &mut GenerateContext,
  ) -> anyhow::Result<DiagnosableResult<GenerateOutput>>;
}
