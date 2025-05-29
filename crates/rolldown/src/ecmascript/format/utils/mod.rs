use itertools::Itertools;
use rolldown_common::ExternalModule;
use rolldown_sourcemap::SourceJoiner;

use crate::{
  ecmascript::ecma_generator::{RenderedModuleSource, RenderedModuleSources},
  types::generator::GenerateContext,
};

pub mod namespace;

pub fn render_factory_parameters(
  ctx: &GenerateContext<'_>,
  externals: &[&ExternalModule],
  has_exports: bool,
) -> String {
  let mut parameters = if has_exports { vec!["exports"] } else { vec![] };
  externals.iter().for_each(|external| {
    let symbol_name = &ctx.chunk.canonical_names[&external.namespace_ref];
    parameters.push(symbol_name.as_str());
  });
  parameters.join(", ")
}

pub fn render_chunk_external_imports<'a>(
  ctx: &'a GenerateContext<'_>,
) -> (String, Vec<&'a ExternalModule>) {
  let mut import_code = String::new();

  let externals = ctx
    .chunk
    .imports_from_external_modules
    .iter()
    .filter_map(|(importee_id, _)| {
      let importee = ctx.link_output.module_table[*importee_id]
        .as_external()
        .expect("Should be external module here");

      let external_module_symbol_name = &ctx.chunk.canonical_names[&importee.namespace_ref];

      if ctx.link_output.used_symbol_refs.contains(&importee.namespace_ref) {
        let to_esm_fn_name = &ctx.chunk.canonical_names[&ctx
          .link_output
          .symbol_db
          .canonical_ref_for(ctx.link_output.runtime.resolve_symbol("__toESM"))];

        import_code.push_str(external_module_symbol_name);
        import_code.push_str(" = ");
        import_code.push_str(to_esm_fn_name);
        import_code.push('(');
        import_code.push_str(external_module_symbol_name);
        import_code.push_str(");\n");
        Some(importee)
      } else if importee.side_effects.has_side_effects() {
        Some(importee)
      } else {
        None
      }
    })
    .collect_vec();

  (import_code, externals)
}

pub fn render_modules_with_peek_runtime_module_at_first<'a>(
  ctx: &GenerateContext<'_>,
  source_joiner: &mut SourceJoiner<'a>,
  module_sources: &'a RenderedModuleSources,
  import_code: String,
) {
  let mut module_sources_peekable = module_sources.iter().peekable();
  match module_sources_peekable.peek() {
    Some(RenderedModuleSource { module_idx, .. })
      if *module_idx == ctx.link_output.runtime.id() =>
    {
      if let RenderedModuleSource { sources: Some(emitted_sources), .. } =
        module_sources_peekable.next().expect("Must have module")
      {
        for source in emitted_sources.iter() {
          source_joiner.append_source(source);
        }
      }
    }
    _ => {}
  }

  source_joiner.append_source(import_code);

  // chunk content
  // TODO indent chunk content for iife format
  module_sources_peekable.for_each(
    |RenderedModuleSource { sources: module_render_output, .. }| {
      if let Some(emitted_sources) = module_render_output {
        for source in emitted_sources.as_ref() {
          source_joiner.append_source(source);
        }
      }
    },
  );
}

pub fn render_chunk_directives<'a, T: Iterator<Item = &'a &'a str>>(directives: T) -> String {
  let mut ret = String::new();
  for d in directives {
    ret.push_str(d);
    if (d).ends_with(';') {
      ret.push('\n');
    } else {
      ret.push_str(";\n");
    }
  }
  ret
}
