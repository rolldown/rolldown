use rolldown_sourcemap::SourceJoiner;

use crate::{ecmascript::ecma_generator::RenderedModuleSources, types::generator::GenerateContext};

pub fn render_app(
  _ctx: &GenerateContext<'_>,
  module_sources: RenderedModuleSources,
  banner: Option<String>,
  footer: Option<String>,
  intro: Option<String>,
  outro: Option<String>,
  hashbang: Option<&str>,
) -> SourceJoiner<'static> {
  let mut source_joiner = SourceJoiner::default();

  if let Some(hashbang) = hashbang {
    source_joiner.append_source(hashbang.to_string());
  }
  if let Some(banner) = banner {
    source_joiner.append_source(banner);
  }

  if let Some(intro) = intro {
    source_joiner.append_source(intro);
  }

  // chunk content
  module_sources.into_iter().for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources {
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
