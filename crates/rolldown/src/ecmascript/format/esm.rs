use rolldown_common::{ChunkKind, WrapKind};
use rolldown_sourcemap::{ConcatSource, RawSource};

use crate::{
  ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::{
    render_chunk_exports::render_chunk_exports, render_chunk_imports::render_chunk_imports,
  },
};

pub fn render_esm(
  ctx: &GenerateContext<'_>,
  module_sources: RenderedModuleSources,
  banner: Option<String>,
  footer: Option<String>,
) -> ConcatSource {
  let mut concat_source = ConcatSource::default();

  if let Some(banner) = banner {
    concat_source.add_source(Box::new(RawSource::new(banner)));
  }

  concat_source.add_source(Box::new(RawSource::new(render_chunk_imports(
    ctx.chunk,
    ctx.link_output,
    ctx.chunk_graph,
    ctx.options,
  ))));

  // chunk content
  module_sources.into_iter().for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources {
        concat_source.add_source(source);
      }
    }
  });

  if let ChunkKind::EntryPoint { module: entry_id, .. } = ctx.chunk.kind {
    let entry_meta = &ctx.link_output.metas[entry_id];
    match entry_meta.wrap_kind {
      WrapKind::Esm => {
        // init_xxx()
        let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();
        let wrapper_ref_name =
          ctx.link_output.symbols.canonical_name_for(*wrapper_ref, &ctx.chunk.canonical_names);
        concat_source.add_source(Box::new(RawSource::new(format!("{wrapper_ref_name}();",))));
      }
      WrapKind::Cjs => {
        // "export default require_xxx();"
        let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();
        let wrapper_ref_name =
          ctx.link_output.symbols.canonical_name_for(*wrapper_ref, &ctx.chunk.canonical_names);
        concat_source
          .add_source(Box::new(RawSource::new(format!("export default {wrapper_ref_name}();\n"))));
      }
      WrapKind::None => {}
    }
  }

  if let Some(exports) =
    render_chunk_exports(ctx.chunk, &ctx.link_output.runtime, ctx.link_output, ctx.options)
  {
    concat_source.add_source(Box::new(RawSource::new(exports)));
  }

  if let Some(footer) = footer {
    concat_source.add_source(Box::new(RawSource::new(footer)));
  }

  concat_source
}
