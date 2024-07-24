use crate::utils::chunk::render_chunk_imports::render_chunk_imports;
use crate::{
  ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::{
    determine_export_mode, get_export_items, render_chunk_exports,
  },
};
use rolldown_common::{ChunkKind, OutputExports};
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
  invoke: bool,
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
  let named_exports = matches!(
    determine_export_mode(&ctx.options.exports, entry_module, &export_items)?,
    OutputExports::Named
  );

  let (import_code, externals) = render_chunk_imports(ctx);

  let (input_args, output_args) =
    render_iife_arguments(&externals, &ctx.options.globals, has_exports && named_exports);

  concat_source.add_source(Box::new(RawSource::new(format!(
    "{}(function({}) {{\n",
    if let Some(name) = &ctx.options.name { format!("var {name} = ") } else { String::new() },
    // TODO handle external imports here.
    input_args
  ))));

  concat_source.add_source(Box::new(RawSource::new(import_code)));

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
  if let Some(exports) = render_chunk_exports(ctx)? {
    concat_source.add_source(Box::new(RawSource::new(exports)));
    if named_exports {
      // We need to add `return exports;` here only if using `named`, because the default value is returned when using `default` in `render_chunk_exports`.
      concat_source.add_source(Box::new(RawSource::new("return exports;".to_string())));
    }
  }

  // iife wrapper end
  if invoke {
    concat_source.add_source(Box::new(RawSource::new(format!("}})({output_args});"))));
  } else {
    concat_source.add_source(Box::new(RawSource::new("})".to_string())));
  }

  if let Some(footer) = footer {
    concat_source.add_source(Box::new(RawSource::new(footer)));
  }

  Ok(concat_source)
}

fn render_iife_arguments(
  externals: &[String],
  globals: &FxHashMap<String, String>,
  exports_key: bool,
) -> (String, String) {
  let mut input_args = if exports_key { vec!["exports".to_string()] } else { vec![] };
  let mut output_args = if exports_key { vec!["{}".to_string()] } else { vec![] };
  externals.iter().for_each(|external| {
    input_args.push(legitimize_identifier_name(external).to_string());
    if let Some(global) = globals.get(external) {
      output_args.push(legitimize_identifier_name(global).to_string());
    } else {
      // TODO add warning for missing global
      output_args.push(legitimize_identifier_name(external).to_string());
    }
  });
  (input_args.join(", "), output_args.join(", "))
}
