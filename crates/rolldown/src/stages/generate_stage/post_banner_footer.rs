use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_sourcemap::{SourceJoiner, SourceMapSource};
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};

use crate::type_alias::IndexInstantiatedChunks;
use crate::utils::shebang::find_shebang_end;

use super::GenerateStage;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn post_banner_footer(
    chunks: &mut IndexInstantiatedChunks,
  ) -> BuildResult<Vec<BuildDiagnostic>> {
    let warnings = chunks
      .par_iter_mut()
      .map(|chunk| {
        let mut chunk_warnings = Vec::new();

        if !matches!(chunk.kind, rolldown_common::InstantiationKind::Ecma(_)) {
          // Only process Ecma chunks
          return Ok(chunk_warnings);
        }
        if chunk.post_banner.is_none() && chunk.post_footer.is_none() {
          // Nothing to do
          return Ok(chunk_warnings);
        }

        let (content, map) = {
          let content = chunk.content.try_as_inner_str()?;

          // Extract shebang if exists
          let (shebang_end, has_shebang) = find_shebang_end(content);

          // Check if post_banner also contains shebang and emit warning
          if has_shebang
            && chunk.post_banner.as_ref().is_some_and(|pb| find_shebang_end(pb.as_str()).1)
          {
            chunk_warnings.push(
              BuildDiagnostic::duplicate_shebang(chunk.preliminary_filename.to_string())
                .with_severity_warning(),
            );
          }

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
            source_joiner.append_source(SourceMapSource::new(rest_content.to_string(), source_map));
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

        Ok(chunk_warnings)
      })
      .collect::<BuildResult<Vec<_>>>()?;

    Ok(warnings.into_iter().flatten().collect())
  }
}
