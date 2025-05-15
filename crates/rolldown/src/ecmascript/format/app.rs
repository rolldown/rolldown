use rolldown_common::AddonRenderContext;
use rolldown_sourcemap::SourceJoiner;

use crate::{
  ecmascript::ecma_generator::{RenderedModuleSource, RenderedModuleSources},
  types::generator::GenerateContext,
};

use super::utils::render_chunk_directives;

#[allow(clippy::needless_pass_by_value)]
pub fn render_app<'code>(
  _ctx: &GenerateContext<'_>,
  addon_render_context: AddonRenderContext<'code>,
  module_sources: &'code RenderedModuleSources,
) -> SourceJoiner<'code> {
  let mut source_joiner = SourceJoiner::default();
  let AddonRenderContext { hashbang, banner, intro, outro, footer, directives } =
    addon_render_context;
  if let Some(hashbang) = hashbang {
    source_joiner.append_source(hashbang);
  }
  if let Some(banner) = banner {
    source_joiner.append_source(banner);
  }

  if !directives.is_empty() {
    source_joiner.append_source(render_chunk_directives(directives.iter()));
    source_joiner.append_source("");
  }

  if let Some(intro) = intro {
    source_joiner.append_source(intro);
  }

  // chunk content
  module_sources.iter().for_each(
    |RenderedModuleSource { sources: module_render_output, .. }| {
      if let Some(emitted_sources) = module_render_output {
        for source in emitted_sources.as_ref() {
          source_joiner.append_source(source);
        }
      }
    },
  );

  if let Some(outro) = outro {
    source_joiner.append_source(outro);
  }

  if let Some(footer) = footer {
    source_joiner.append_source(footer);
  }

  source_joiner
}
