use rolldown_error::BuildResult;
use rolldown_sourcemap::{shift_sourcemap_lines, SourceJoiner, SourceMapSource};
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};

use crate::type_alias::IndexInstantiatedChunks;
use crate::utils::shebang::find_shebang_end;

use super::GenerateStage;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn post_banner_footer(chunks: &mut IndexInstantiatedChunks) -> BuildResult<()> {
    chunks.par_iter_mut().try_for_each(|chunk| {
      if !matches!(chunk.kind, rolldown_common::InstantiationKind::Ecma(_)) {
        // Only process Ecma chunks
        return Ok(());
      }
      if chunk.post_banner.is_none() && chunk.post_footer.is_none() {
        // Nothing to do
        return Ok(());
      }

      let (content, map) = {
        let content = chunk.content.try_as_inner_str()?;

        // Extract shebang if exists
        let (shebang_end, has_shebang) = find_shebang_end(content);

        let mut source_joiner = SourceJoiner::default();

        // Add shebang first if it exists
        if has_shebang {
          source_joiner.append_source(content[..shebang_end].trim_end()); // Trim to avoid extra newlines
        }

        // Then add post_banner
        if let Some(post_banner) = &chunk.post_banner {
          source_joiner.append_source(post_banner.as_str());
        }

        let rest_content = &content[shebang_end..];
        // Add the rest of the content
        if let Some(source_map) = chunk.map.take() {
          // If we extracted a shebang, we need to adjust the source map's generated line numbers
          // because the source map was generated for the full content (including shebang),
          // but we're now treating rest_content as if it starts at line 0.
          // We need to shift the generated line numbers down by the number of lines in the shebang.
          let adjusted_source_map = if has_shebang {
            // Count the number of lines in the shebang portion
            let shebang_lines = content[..shebang_end].matches('\n').count() as i32;
            shift_sourcemap_lines(&source_map, -shebang_lines)
          } else {
            source_map
          };
          source_joiner.append_source(SourceMapSource::new(rest_content.to_string(), adjusted_source_map));
        } else {
          source_joiner.append_source(rest_content);
        }

        if let Some(post_footer) = &chunk.post_footer {
          source_joiner.append_source(post_footer.as_str());
        }

        source_joiner.join()
      };
      chunk.content = content.into();
      chunk.map = map;

      Ok(())
    })
  }
}
