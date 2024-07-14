use rolldown_common::{Chunk, NormalizedBundlerOptions, PreliminaryAsset};

use crate::{chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput};

pub struct GenerateContext<'a> {
  pub chunk: &'a Chunk,
  pub options: &'a NormalizedBundlerOptions,
  pub link_output: &'a LinkStageOutput,
  pub chunk_graph: &'a ChunkGraph,
}

pub trait Generator {
  async fn render_preliminary_assets(
    ctx: &GenerateContext,
  ) -> anyhow::Result<Vec<PreliminaryAsset>>;
}
