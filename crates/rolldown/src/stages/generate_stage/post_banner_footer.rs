use rolldown_error::BuildResult;
use rolldown_sourcemap::{SourceJoiner, SourceMapSource};
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};

use crate::type_alias::IndexInstantiatedChunks;

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

      let content = chunk.content.try_as_inner_str()?.to_string();

      // Check if content starts with a shebang
      let (shebang, rest_content) = if content.starts_with("#!") {
        if let Some(newline_pos) = content.find('\n') {
          (&content[..=newline_pos], &content[newline_pos + 1..])
        } else {
          // If no newline found, treat the whole content as shebang
          (content.as_str(), "")
        }
      } else {
        ("", content.as_str())
      };

      let mut source_joiner = SourceJoiner::default();

      // Add shebang first if it exists
      if !shebang.is_empty() {
        source_joiner.append_source(shebang.to_string());
      }

      // Then add post_banner
      if let Some(post_banner) = &chunk.post_banner {
        source_joiner.append_source(post_banner.clone());
      }

      // Add the rest of the content
      if let Some(source_map) = chunk.map.take() {
        source_joiner.append_source(SourceMapSource::new(rest_content.to_string(), source_map));
      } else {
        source_joiner.append_source(rest_content.to_string());
      }

      if let Some(post_footer) = &chunk.post_footer {
        source_joiner.append_source(post_footer.clone());
      }

      let (content, map) = source_joiner.join();
      chunk.content = content.into();
      chunk.map = map;

      Ok(())
    })
  }
}
