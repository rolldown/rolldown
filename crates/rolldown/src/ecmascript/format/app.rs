use rolldown_sourcemap::{ConcatSource, RawSource};

use crate::{
  append_injection, ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
};

pub fn render_app(
  _ctx: &GenerateContext<'_>,
  module_sources: RenderedModuleSources,
  banner: Option<String>,
  footer: Option<String>,
  intro: Option<String>,
  outro: Option<String>,
) -> ConcatSource {
  let mut concat_source = ConcatSource::default();

  append_injection!(concat_source, banner, intro);

  // chunk content
  module_sources.into_iter().for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources {
        concat_source.add_source(source);
      }
    }
  });

  append_injection!(concat_source, footer, outro);

  concat_source
}
