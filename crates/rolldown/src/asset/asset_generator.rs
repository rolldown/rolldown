use crate::types::generator::{GenerateContext, GenerateOutput, Generator};

use anyhow::Result;
use rolldown_common::{InstantiatedChunk, InstantiationKind, StrOrBytes};
use rolldown_error::BuildResult;
use rolldown_std_utils::OptionExt;

pub struct AssetGenerator;

impl Generator for AssetGenerator {
  #[allow(clippy::too_many_lines)]
  async fn instantiate_chunk(ctx: &mut GenerateContext<'_>) -> Result<BuildResult<GenerateOutput>> {
    let asset_modules = ctx
      .chunk
      .modules
      .iter()
      .filter_map(|&id| ctx.link_output.module_table.modules[id].as_normal())
      .filter(|m| m.asset_view.is_some())
      .collect::<Vec<_>>();

    let mut instantiated_chunks = Vec::with_capacity(asset_modules.len());

    for asset_module in asset_modules {
      let asset_view = asset_module.asset_view.unpack_ref();
      let preliminary_filename =
        ctx.chunk.asset_preliminary_filenames.get(&asset_module.idx).unpack();
      let file_path =
        ctx.options.cwd.as_path().join(&ctx.options.out_dir).join(preliminary_filename.as_str());
      let file_dir = file_path.parent().expect("chunk file name should have a parent");
      instantiated_chunks.push(InstantiatedChunk {
        origin_chunk: ctx.chunk_idx,
        content: StrOrBytes::Bytes(asset_view.source.to_vec()),
        map: None,
        kind: InstantiationKind::None,
        augment_chunk_hash: None,
        file_dir: file_dir.to_path_buf(),
        preliminary_filename: preliminary_filename.clone(),
      });
    }

    Ok(Ok(GenerateOutput {
      chunks: instantiated_chunks,
      warnings: std::mem::take(&mut ctx.warnings),
    }))
  }
}
