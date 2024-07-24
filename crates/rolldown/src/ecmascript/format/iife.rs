use crate::{
  ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::{
    render_chunk_exports::{determine_export_mode, get_export_items},
    render_wrapper::render_wrapper,
  },
};
use rolldown_common::ChunkKind;
use rolldown_error::DiagnosableResult;
use rolldown_sourcemap::{ConcatSource, RawSource};
use rolldown_utils::ecma_script::legitimize_identifier_name;
use rustc_hash::FxHashMap;

// TODO refactor it to `wrap.rs` to reuse it for other formats (e.g. amd, umd).
pub fn render_iife(
  ctx: &mut GenerateContext<'_>,
  module_sources: RenderedModuleSources,
  banner: Option<String>,
  footer: Option<String>,
) -> DiagnosableResult<ConcatSource> {
  let mut concat_source = ConcatSource::default();

  if let Some(banner) = banner {
    concat_source.add_source(Box::new(RawSource::new(banner)));
  }
  // iife wrapper start
  let export_items = get_export_items(ctx.chunk, ctx.link_output);
  let has_exports = !export_items.is_empty();
  // Since before rendering the `determine_export_mode` runs, `unwrap` here won't cause panic.
  // FIXME do not call `determine_export_mode` twice
  let entry_module = match ctx.chunk.kind {
    ChunkKind::EntryPoint { module, .. } => {
      &ctx.link_output.module_table.modules[module].as_ecma().expect("should be ecma module")
    }
    ChunkKind::Common => unreachable!("iife should be entry point chunk"),
  };
  let export_mode = determine_export_mode(&ctx.options.exports, entry_module, &export_items)?;

  let assignee =
    if let Some(name) = &ctx.options.name { format!("var {} = ", name) } else { "".to_string() };

  let (begin_wrapper, end_wrapper, externals) = render_wrapper(ctx, &export_mode, true)?;

  let begging = format!("{assignee}{begin_wrapper}");

  concat_source.add_source(Box::new(RawSource::new(begging)));

  // TODO indent chunk content for the wrapper function
  module_sources.into_iter().for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources {
        concat_source.add_source(source);
      }
    }
  });

  let arguments = render_iife_arguments(
    &externals,
    &ctx.options.globals,
    has_exports && matches!(export_mode, rolldown_common::OutputExports::Named),
  );

  let ending = format!("{end_wrapper}({arguments});");

  concat_source.add_source(Box::new(RawSource::new(ending)));

  if let Some(footer) = footer {
    concat_source.add_source(Box::new(RawSource::new(footer)));
  }

  Ok(concat_source)
}

fn render_iife_arguments(
  externals: &[String],
  globals: &FxHashMap<String, String>,
  exports_key: bool,
) -> String {
  let mut output_args = if exports_key { vec!["{}".to_string()] } else { vec![] };
  externals.iter().for_each(|external| {
    if let Some(global) = globals.get(external) {
      output_args.push(legitimize_identifier_name(global).to_string());
    } else {
      // TODO add warning for missing global
      output_args.push(legitimize_identifier_name(external).to_string());
    }
  });
  output_args.join(", ")
}
