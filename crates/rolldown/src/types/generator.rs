use rolldown_common::{Chunk, ChunkIdx, NormalizedBundlerOptions, PreliminaryAsset};
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
}

pub struct GenerateOutput {
  pub assets: Vec<PreliminaryAsset>,
  pub warnings: Vec<BuildDiagnostic>,
}

pub trait Generator {
  async fn render_preliminary_assets(
    ctx: &mut GenerateContext,
  ) -> anyhow::Result<DiagnosableResult<GenerateOutput>>;
}
