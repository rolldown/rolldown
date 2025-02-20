use rolldown_sourcemap::SourceJoiner;

use crate::{ecmascript::ecma_generator::RenderedModuleSources, types::generator::GenerateContext};

pub fn render_app<'code>(
  _ctx: &GenerateContext<'_>,
  hashbang: Option<&'code str>,
  banner: Option<&'code str>,
  intro: Option<&'code str>,
  outro: Option<&'code str>,
  footer: Option<&'code str>,
  module_sources: &'code RenderedModuleSources,
) -> SourceJoiner<'code> {
  let mut source_joiner = SourceJoiner::default();

  if let Some(hashbang) = hashbang {
    source_joiner.append_source(hashbang);
  }
  if let Some(banner) = banner {
    source_joiner.append_source(banner);
  }

  if let Some(intro) = intro {
    source_joiner.append_source(intro);
  }

  // chunk content
  module_sources.iter().for_each(|(_, _, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources.as_ref() {
        source_joiner.append_source(source);
      }
    }
  });

  if let Some(outro) = outro {
    source_joiner.append_source(outro);
  }

  if let Some(footer) = footer {
    source_joiner.append_source(footer);
  }

  source_joiner
}
