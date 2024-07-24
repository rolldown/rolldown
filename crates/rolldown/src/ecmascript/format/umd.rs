use crate::ecmascript::ecma_generator::RenderedModuleSources;
use crate::ecmascript::format::amd::render_amd_arguments;
use crate::types::generator::GenerateContext;
use crate::utils::chunk::determine_use_strict::determine_use_strict;
use crate::utils::chunk::render_chunk_exports::{determine_export_mode, get_export_items};
use crate::utils::chunk::render_wrapper::render_wrapper;
use arcstr::ArcStr;
use rolldown_common::{ChunkKind, OutputExports};
use rolldown_error::DiagnosableResult;
use rolldown_sourcemap::{ConcatSource, RawSource};
use rolldown_utils::ecma_script::legitimize_identifier_name;
use rustc_hash::FxHashMap;

pub fn render_umd(
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

  // iife wrapper start
  let export_items = get_export_items(ctx.chunk, ctx.link_output);
  let has_exports = !export_items.is_empty();

  let entry_module = match ctx.chunk.kind {
    ChunkKind::EntryPoint { module, .. } => {
      &ctx.link_output.module_table.modules[module].as_ecma().expect("should be ecma module")
    }
    ChunkKind::Common => unreachable!("umd should be entry point chunk"),
  };
  let export_mode = determine_export_mode(&ctx.options.exports, entry_module, &export_items)?;

  let (begin_wrapper, end_wrapper, externals) =
    render_wrapper(ctx, &export_mode, determine_use_strict(ctx), intro, outro);

  let name = &ctx.options.name;

  if let Some(name) = name {
    let (head, tail) = render_umd_wrapper(
      ctx,
      &externals,
      name,
      has_exports && matches!(export_mode, OutputExports::Named),
    );

    let begging = format!("{head}{begin_wrapper}");

    concat_source.add_source(Box::new(RawSource::new(begging)));

    // TODO indent chunk content for the wrapper function
    module_sources.into_iter().for_each(|(_, _, module_render_output)| {
      if let Some(emitted_sources) = module_render_output {
        for source in emitted_sources {
          concat_source.add_source(source);
        }
      }
    });

    let ending = format!("{end_wrapper}{tail}");

    concat_source.add_source(Box::new(RawSource::new(ending)));

    if let Some(footer) = footer {
      concat_source.add_source(Box::new(RawSource::new(footer)));
    }

    Ok(concat_source)
  } else {
    // TODO use `Diagnostic` to report error
    panic!("`output.name` should be specified for umd output");
  }
}

pub fn render_umd_wrapper(
  ctx: &GenerateContext<'_>,
  externals: &[(String, bool)],
  name: &str,
  exports_key: bool,
) -> (String, String) {
  let cjs_args = render_cjs_arguments_umd(externals, exports_key);
  let iife_args =
    render_iife_arguments_umd(externals, &ArcStr::from("root"), &ctx.options.globals, exports_key);
  let amd_args = render_amd_arguments(externals);

  (
    format!(
      "(function (global, factory) {{\n\
    typeof exports === 'object' && typeof module !== 'undefined' ? factory({cjs_args}) :\n\
	typeof define === 'function' && define.amd ? define([{amd_args}], factory) :\n\
	(global = typeof globalThis !== 'undefined' ? globalThis : global || self, global.{name} = factory({iife_args}));\n\
}})(this, ",
    ),
    ");".to_string(),
  )
}

fn render_cjs_arguments_umd(externals: &[(String, bool)], exports_key: bool) -> String {
  let mut output_args = if exports_key { vec!["exports".to_string()] } else { vec![] };
  externals.iter().for_each(|(external, _)| {
    output_args.push(format!("require(\"{external}\")"));
  });
  output_args.join(", ")
}

fn render_iife_arguments_umd(
  externals: &[(String, bool)],
  name: &ArcStr,
  globals: &FxHashMap<String, String>,
  exports_key: bool,
) -> String {
  let mut output_args = if exports_key { vec![format!("global.{name} = {{}}")] } else { vec![] };
  externals.iter().for_each(|(external, non_empty)| {
    if *non_empty {
      let syntax = format!(
        "global.{}",
        if let Some(global) = globals.get(external) {
          legitimize_identifier_name(global).to_string()
        } else {
          // TODO add warning for missing global
          legitimize_identifier_name(external).to_string()
        }
      );
      output_args.push(syntax);
    }
  });
  output_args.join(", ")
}
