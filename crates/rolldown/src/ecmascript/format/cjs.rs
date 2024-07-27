use rolldown_error::DiagnosableResult;
use rolldown_sourcemap::{ConcatSource, RawSource};

use crate::{
  append_injection,
  ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::{
    collect_render_chunk_imports::{
      collect_render_chunk_imports, RenderImportDeclarationSpecifier,
    },
    determine_use_strict::determine_use_strict,
    render_chunk_exports::render_chunk_exports,
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

  append_injection!(concat_source, banner);

  if determine_use_strict(ctx) {
    concat_source.add_source(Box::new(RawSource::new("\"use strict\";".to_string())));
  }

  append_injection!(concat_source, intro);

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

  concat_source.add_source(Box::new(RawSource::new(render_cjs_chunk_imports(ctx))));

  // chunk content
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

  append_injection!(concat_source, outro, footer);

  Ok(concat_source)
}

fn render_cjs_chunk_imports(ctx: &GenerateContext<'_>) -> String {
  let render_import_stmts =
    collect_render_chunk_imports(ctx.chunk, ctx.link_output, ctx.chunk_graph);

  let mut s = String::new();
  render_import_stmts.iter().for_each(|stmt| {
    let require_path_str = format!("require(\"{}\")", &stmt.path);
    match &stmt.specifiers {
      RenderImportDeclarationSpecifier::ImportSpecifier(specifiers) => {
        if specifiers.is_empty() {
          s.push_str(&format!("{require_path_str};\n"));
        } else {
          let specifiers = specifiers
            .iter()
            .map(|specifier| {
              if let Some(alias) = &specifier.alias {
                format!("{}: {alias}", specifier.imported)
              } else {
                specifier.imported.to_string()
              }
            })
            .collect::<Vec<_>>();
          s.push_str(&format!(
            "const {{ {} }} = {};\n",
            specifiers.join(", "),
            if stmt.is_external {
              let to_esm_fn_name = &ctx.chunk.canonical_names[&ctx
                .link_output
                .symbols
                .par_canonical_ref_for(ctx.link_output.runtime.resolve_symbol("__toESM"))];

              format!("{to_esm_fn_name}({require_path_str})")
            } else {
              require_path_str
            }
          ));
        }
      }
      RenderImportDeclarationSpecifier::ImportStarSpecifier(alias) => {
        s.push_str(&format!(
          "const {alias} = {};\n",
          if stmt.is_external {
            let to_esm_fn_name = &ctx.chunk.canonical_names[&ctx
              .link_output
              .symbols
              .par_canonical_ref_for(ctx.link_output.runtime.resolve_symbol("__toESM"))];

            format!("{to_esm_fn_name}({require_path_str})")
          } else {
            require_path_str
          }
        ));
      }
    }
  });

  s
}
