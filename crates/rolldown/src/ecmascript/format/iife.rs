use rolldown_sourcemap::{ConcatSource, RawSource};

use crate::{
  ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::{get_export_items, render_chunk_exports},
};

pub fn render_iife(
  ctx: &GenerateContext<'_>,
  module_sources: RenderedModuleSources,
  banner: Option<String>,
  footer: Option<String>,
) -> ConcatSource {
  let mut concat_source = ConcatSource::default();

  if let Some(banner) = banner {
    concat_source.add_source(Box::new(RawSource::new(banner)));
  }
  // iife wrapper start
  let has_exports = !get_export_items(ctx.chunk, ctx.link_output).is_empty();
  if let Some(name) = &ctx.options.name {
    concat_source.add_source(Box::new(RawSource::new(format!(
      "var {name} = (function({}) {{\n",
      if has_exports { "exports" } else { "" }
    ))));
  } else {
    concat_source.add_source(Box::new(RawSource::new("(function() {\n".to_string())));
  }

  // TODO iife imports

  // chunk content
  // TODO indent chunk content for iife format
  module_sources.into_iter().for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources {
        concat_source.add_source(source);
      }
    }
  });

  // iife exports
  if let Some(exports) =
    render_chunk_exports(ctx.chunk, &ctx.link_output.runtime, ctx.link_output, ctx.options)
  {
    concat_source.add_source(Box::new(RawSource::new(exports)));
    concat_source.add_source(Box::new(RawSource::new("return exports;".to_string())));
  }

  // iife wrapper end
  concat_source
    .add_source(Box::new(RawSource::new(format!("}})({});", if has_exports { "{}" } else { "" }))));

  if let Some(footer) = footer {
    concat_source.add_source(Box::new(RawSource::new(footer)));
  }

  concat_source
}
