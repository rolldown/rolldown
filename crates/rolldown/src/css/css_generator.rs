use crate::{
  types::generator::{GenerateContext, Generator},
  utils::{chunk::generate_rendered_chunk, render_ecma_module::render_ecma_module},
};

use anyhow::Result;
use rolldown_common::{
  AssetMeta, EcmaAssetMeta, ModuleId, ModuleIdx, OutputFormat, PreliminaryAsset, RenderedModule,
};

pub struct CssGenerator;

impl Generator for CssGenerator {
  #[allow(clippy::too_many_lines)]
  async fn render_preliminary_assets<'a>(
    ctx: &GenerateContext<'a>,
  ) -> Result<Vec<PreliminaryAsset>> {
    Ok(vec![])
  }
}
