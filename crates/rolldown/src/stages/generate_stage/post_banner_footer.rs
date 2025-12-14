use rolldown_sourcemap::{SourceJoiner, SourceMapSource};
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};

use crate::type_alias::IndexInstantiatedChunks;

use super::GenerateStage;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn post_banner_footer(chunks: &mut IndexInstantiatedChunks) {
    chunks.par_iter_mut().for_each(|chunk| {
      if chunk.post_banner.is_none() && chunk.post_footer.is_none() {
        // Nothing to do
        return;
      }
      let Ok(content) = chunk.content.try_as_inner_str() else {
        // TODO: what should we do here?
        return;
      };

      let mut source_joiner = SourceJoiner::default();

      if let Some(post_banner) = &chunk.post_banner {
        source_joiner.append_source(post_banner.clone());
      }

      if let Some(source_map) = chunk.map.take() {
        source_joiner.append_source(SourceMapSource::new(content.to_string(), source_map));
      } else {
        source_joiner.append_source(content.to_string());
      }

      if let Some(post_footer) = &chunk.post_footer {
        source_joiner.append_source(post_footer.clone());
      }

      let (content, map) = source_joiner.join();
      chunk.content = content.into();
      chunk.map = map;
    });
  }
}
