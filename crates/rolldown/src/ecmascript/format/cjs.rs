use rolldown_error::DiagnosableResult;
use rolldown_sourcemap::{ConcatSource, RawSource};

use crate::{
  ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::{
    determine_use_strict::determine_use_strict, render_chunk_exports::render_chunk_exports,
    render_chunk_imports::render_chunk_imports,
  },
};

pub fn render_cjs(
  ctx: &mut GenerateContext<'_>,
  module_sources: RenderedModuleSources,
  banner: Option<String>,
  footer: Option<String>,
  intro: Option<String>,
  outro: Option<String>,
) -> DiagnosableResult<ConcatSource> {
  let mut concat_source = ConcatSource::default();

  if let Some(banner) = banner {
    concat_source.add_source(Box::new(RawSource::new(banner)));
  }

  if determine_use_strict(ctx) {
    concat_source.add_source(Box::new(RawSource::new("\"use strict\";".to_string())));
  }

  if let Some(intro) = intro {
    if !intro.is_empty() {
      concat_source.add_source(Box::new(RawSource::new(intro)));
    }
  }

  // Runtime module should be placed before the generated `requires` in CJS format.
  // Because, we might need to generate `__toESM(require(...))` that relies on the runtime module.
  let mut module_sources_peekable = module_sources.into_iter().peekable();
  match module_sources_peekable.peek() {
    Some((id, _, _)) if *id == ctx.link_output.runtime.id() => {
      if let (_, _module_id, Some(emitted_sources)) =
        module_sources_peekable.next().expect("Must have module")
      {
        for source in emitted_sources {
          concat_source.add_source(source);
        }
      }
    }
    _ => {}
  }

  let (imports, _) = render_chunk_imports(ctx);

  concat_source.add_source(Box::new(RawSource::new(imports)));

  // chunk content
  // TODO add indents
  module_sources_peekable.for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources {
        concat_source.add_source(source);
      }
    }
  });

  if let Some(exports) = render_chunk_exports(ctx)? {
    concat_source.add_source(Box::new(RawSource::new(exports)));
  }

  if let Some(outro) = outro {
    if !outro.is_empty() {
      concat_source.add_source(Box::new(RawSource::new(outro)));
    }
  }

  if let Some(footer) = footer {
    concat_source.add_source(Box::new(RawSource::new(footer)));
  }

  Ok(concat_source)
}
