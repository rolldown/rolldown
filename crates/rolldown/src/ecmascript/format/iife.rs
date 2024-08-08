use crate::utils::chunk::collect_render_chunk_imports::{
  collect_render_chunk_imports, RenderImportDeclarationSpecifier,
};
use crate::utils::chunk::namespace_marker::render_namespace_markers;
use crate::{
  ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::{
    determine_export_mode::determine_export_mode,
    determine_use_strict::determine_use_strict,
    render_chunk_exports::{get_export_items, render_chunk_exports},
  },
};
use arcstr::ArcStr;
use rolldown_common::{ChunkKind, OutputExports};
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_sourcemap::{ConcatSource, RawSource};
use rolldown_utils::ecma_script::legitimize_identifier_name;

// TODO refactor it to `wrap.rs` to reuse it for other formats (e.g. amd, umd).
pub fn render_iife(
  ctx: &mut GenerateContext<'_>,
  module_sources: RenderedModuleSources,
  banner: Option<String>,
  footer: Option<String>,
  intro: Option<String>,
  outro: Option<String>,
  invoke: bool,
) -> DiagnosableResult<ConcatSource> {
  let mut concat_source = ConcatSource::default();

  if let Some(banner) = banner {
    concat_source.add_source(Box::new(RawSource::new(banner)));
  }

  // iife wrapper start
  let export_items = get_export_items(ctx.chunk, ctx.link_output);
  let has_exports = !export_items.is_empty();
  let has_default_export = export_items.iter().any(|(name, _)| name.as_str() == "default");
  let entry_module = match ctx.chunk.kind {
    ChunkKind::EntryPoint { module, .. } => {
      &ctx.link_output.module_table.modules[module].as_ecma().expect("should be ecma module")
    }
    ChunkKind::Common => unreachable!("iife should be entry point chunk"),
  };
  let export_mode = determine_export_mode(ctx, entry_module, &export_items)?;
  let named_exports = matches!(&export_mode, OutputExports::Named);

  let (import_code, externals) = render_iife_chunk_imports(ctx);

  let (input_args, output_args) =
    render_iife_arguments(ctx, &externals, has_exports && named_exports);

  concat_source.add_source(Box::new(RawSource::new(format!(
    "{}(function({}) {{\n",
    // Only IIFEs with exports requires the assignment.
    if has_exports {
      if let Some(name) = &ctx.options.name {
        format!("var {name} = ")
      } else {
        ctx
          .warnings
          .push(BuildDiagnostic::missing_name_option_for_iife_export().with_severity_warning());
        String::new()
      }
    } else {
      String::new()
    },
    input_args
  ))));

  if determine_use_strict(ctx) {
    concat_source.add_source(Box::new(RawSource::new("\"use strict\";".to_string())));
  }

  if let Some(intro) = intro {
    concat_source.add_source(Box::new(RawSource::new(intro)));
  }

  if named_exports {
    if let Some(marker) =
      render_namespace_markers(&ctx.options.es_module, has_default_export, false)
    {
      concat_source.add_source(Box::new(RawSource::new(marker.into())));
    }
  }

  concat_source.add_source(Box::new(RawSource::new(import_code)));

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
  if let Some(exports) = render_chunk_exports(ctx, Some(&export_mode)) {
    concat_source.add_source(Box::new(RawSource::new(exports)));
  }

  if let Some(outro) = outro {
    concat_source.add_source(Box::new(RawSource::new(outro)));
  }

  if named_exports && has_exports {
    // We need to add `return exports;` here only if using `named`, because the default value is returned when using `default` in `render_chunk_exports`.
    concat_source.add_source(Box::new(RawSource::new("return exports;".to_string())));
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

// Handling external imports needs to modify the arguments of the wrapper function.
fn render_iife_chunk_imports(ctx: &GenerateContext<'_>) -> (String, Vec<String>) {
  let render_import_stmts =
    collect_render_chunk_imports(ctx.chunk, ctx.link_output, ctx.chunk_graph);

  let mut s = String::new();
  let externals: Vec<String> = render_import_stmts
    .iter()
    .filter_map(|stmt| {
      let require_path_str = &stmt.path;
      match &stmt.specifiers {
        RenderImportDeclarationSpecifier::ImportSpecifier(specifiers) => {
          // Empty specifiers can be ignored in IIFE.
          if specifiers.is_empty() {
            None
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
              legitimize_identifier_name(&stmt.path)
            ));
            Some(require_path_str.to_string())
          }
        }
        RenderImportDeclarationSpecifier::ImportStarSpecifier(alias) => {
          s.push_str(&format!("const {alias} = {};\n", legitimize_identifier_name(&stmt.path)));
          Some(require_path_str.to_string())
        }
      }
    })
    .collect();

  (s, externals)
}

fn render_iife_arguments(
  ctx: &mut GenerateContext<'_>,
  externals: &[String],
  exports_key: bool,
) -> (String, String) {
  let mut input_args = if exports_key { vec!["exports".to_string()] } else { vec![] };
  let mut output_args = if exports_key { vec!["{}".to_string()] } else { vec![] };
  let globals = &ctx.options.globals;
  externals.iter().for_each(|external| {
    input_args.push(legitimize_identifier_name(external).to_string());
    if let Some(global) = globals.get(external) {
      output_args.push(legitimize_identifier_name(global).to_string());
    } else {
      let target = legitimize_identifier_name(external).to_string();
      ctx.warnings.push(
        BuildDiagnostic::missing_global_name(ArcStr::from(external), ArcStr::from(&target))
          .with_severity_warning(),
      );
      output_args.push(target.to_string());
    }
  });
  (input_args.join(", "), output_args.join(", "))
}
