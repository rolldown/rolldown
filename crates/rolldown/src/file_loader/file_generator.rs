use rolldown_common::{InstantiatedChunk, InstantiationKind};
use rolldown_error::BuildResult;
use rolldown_std_utils::OptionExt;

use crate::types::generator::{GenerateContext, GenerateOutput, Generator};
use anyhow::Result;

pub struct FileGenerator;

impl Generator for FileGenerator {
  async fn instantiate_chunk<'a>(
    ctx: &mut GenerateContext<'a>,
  ) -> Result<BuildResult<GenerateOutput>> {
    let file_modules = ctx
      .chunk
      .modules
      .iter()
      .filter_map(|&id| ctx.link_output.module_table.modules[id].as_normal())
      .filter(|m| m.file_view.is_some())
      .collect::<Vec<_>>();

    let mut instantiated_chunks = vec![];

    for file_module in file_modules {
      let file_view = file_module.file_view.unpack_ref();

      let preliminary_filename =
        ctx.chunk.file_preliminary_filenames.get(&file_module.idx).unpack();

      let file_path =
        ctx.options.cwd.as_path().join(&ctx.options.dir).join(preliminary_filename.as_str());

      let file_dir = file_path.parent().expect("chunk file name should have a parent");
      instantiated_chunks.push(InstantiatedChunk {
        origin_chunk: ctx.chunk_idx,
        content: file_view.source.to_string(),
        map: None,
        meta: InstantiationKind::None,
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
