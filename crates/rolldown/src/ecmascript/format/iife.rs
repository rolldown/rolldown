use crate::{
  ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::{
    determine_export_mode, get_export_items, render_chunk_exports,
  },
};
use rolldown_common::OutputExports;
use rolldown_sourcemap::{ConcatSource, RawSource};

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
  // Since before rendering the `determine_export_mode` runs, `unwrap` here won't cause panic.
  // FIXME do not call `determine_export_mode` twice
  let named_exports = matches!(
    determine_export_mode(ctx.chunk, &ctx.options.exports, ctx.link_output).unwrap(),
    OutputExports::Named
  );

  concat_source.add_source(Box::new(RawSource::new(format!(
    "{}(function({}) {{\n",
    if let Some(name) = &ctx.options.name { format!("var {name} = ") } else { String::new() },
    // TODO handle external imports here.
    if has_exports && named_exports { "exports" } else { "" }
  ))));

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
    if named_exports {
      // We need to add `return exports;` here only if using `named`, because the default value is returned when using `default` in `render_chunk_exports`.
      concat_source.add_source(Box::new(RawSource::new("return exports;".to_string())));
    }
  }

  // iife wrapper end
  concat_source.add_source(Box::new(RawSource::new(format!(
    "}})({});",
    if has_exports && named_exports { "{}" } else { "" }
  ))));

  if let Some(footer) = footer {
    concat_source.add_source(Box::new(RawSource::new(footer)));
  }

  concat_source
}
