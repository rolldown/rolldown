use rolldown_sourcemap::{ConcatSource, RawSource};

use crate::{ecmascript::ecma_generator::RenderedModuleSources, types::generator::GenerateContext};

pub fn render_app(
  _ctx: &GenerateContext<'_>,
  module_sources: RenderedModuleSources,
  banner: Option<String>,
  footer: Option<String>,
  intro: Option<String>,
  outro: Option<String>,
) -> ConcatSource {
  let mut concat_source = ConcatSource::default();

  if let Some(banner) = banner {
    concat_source.add_source(Box::new(RawSource::new(banner)));
  }

  if let Some(intro) = intro {
    concat_source.add_source(Box::new(RawSource::new(intro)));
  }

  // chunk content
  module_sources.into_iter().for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources {
        concat_source.add_source(source);
      }
    }
  });

  if let Some(outro) = outro {
    concat_source.add_source(Box::new(RawSource::new(outro)));
  }

  if let Some(footer) = footer {
    concat_source.add_source(Box::new(RawSource::new(footer)));
  }

  concat_source
}
