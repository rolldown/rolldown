use crate::ecmascript::format::iife::render_iife_factory;
use crate::{
  ecmascript::{
    ecma_generator::RenderedModuleSources, format::utils::external_module::ExternalModules,
  },
  types::generator::GenerateContext,
  utils::chunk::{
    collect_render_chunk_imports::{
      collect_render_chunk_imports, RenderImportDeclarationSpecifier,
    },
    determine_export_mode::determine_export_mode,
    determine_use_strict::determine_use_strict,
    namespace_marker::render_namespace_markers,
    render_chunk_exports::{get_export_items, render_chunk_exports},
  },
};
use rolldown_common::{ChunkKind, OutputExports, OutputFormat};
use rolldown_error::DiagnosableResult;
use rolldown_sourcemap::ConcatSource;
use rolldown_utils::ecma_script::legitimize_identifier_name;

/// The main function for rendering the IIFE format chunks.
/// The factory, e.g. in UMD, it is the factory function. In iife, it is the declaration / assignment.
/// The caller, e.g. in UMD and AMD, it should end up immediately; in IIFE, it should be passed with invoke arguments.
///
/// In Rollup, it uses `render_interop`, as the `magic_string` supports prepend and append. It is more vivid, but we don't have the functionality for `ConcatSource` yet.
pub fn render_wrapper_function(
  ctx: &mut GenerateContext<'_>,
  module_sources: RenderedModuleSources,
  banner: Option<String>,
  footer: Option<String>,
  intro: Option<String>,
  outro: Option<String>,
) -> DiagnosableResult<ConcatSource> {
  let mut concat_source = ConcatSource::default();

  concat_source.add_optional_raw_string(banner);

  // iife wrapper start

  // Analyze the export information of the chunk.
  let export_items = get_export_items(ctx.chunk, ctx.link_output);
  let has_exports = !export_items.is_empty();
  let has_default_export = export_items.iter().any(|(name, _)| name.as_str() == "default");

  let entry_module = match ctx.chunk.kind {
    ChunkKind::EntryPoint { module, .. } => {
      &ctx.link_output.module_table.modules[module].as_ecma().expect("Should be ecma module.")
    }
    ChunkKind::Common => unreachable!("Wrapper function should be entry point chunk."),
  };

  // We need to transform the `OutputExports::Auto` to suitable `OutputExports`.
  let export_mode = determine_export_mode(ctx, entry_module, &export_items)?;

  let named_exports = matches!(&export_mode, OutputExports::Named);

  // It is similar to CJS.
  let (import_code, externals) = render_wrapper_chunk_imports(ctx);

  let (factory, caller) = render_factory(ctx, &export_mode, has_exports, &externals)?;

  concat_source
    .add_raw_string(format!("{factory}(function({}) {{\n", externals.as_args(named_exports)));

  if determine_use_strict(ctx) {
    concat_source.add_raw_string("\"use strict\";".to_string());
  }

  concat_source.add_optional_raw_string(intro);

  if named_exports {
    let marker = render_namespace_markers(&ctx.options.es_module, has_default_export, false);
    concat_source.add_optional_raw_string(marker.map(ToString::to_string));
  }

  concat_source.add_raw_string(import_code);

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
  concat_source.add_optional_raw_string(render_chunk_exports(ctx, Some(&export_mode)));

  concat_source.add_optional_raw_string(outro);

  if named_exports && has_exports && !ctx.options.extend {
    // We need to add `return exports;` here only if using `named`, because the default value is returned when using `default` in `render_chunk_exports`.
    concat_source.add_raw_string("return exports;".to_string());
  }

  concat_source.add_raw_string(format!("}}){caller};"));

  concat_source.add_optional_raw_string(footer);

  Ok(concat_source)
}

/// Handling external imports needs to modify the arguments of the wrapper function.
/// They share the same logic in iife, umd, and amd.
fn render_wrapper_chunk_imports(ctx: &GenerateContext<'_>) -> (String, ExternalModules) {
  let render_import_stmts =
    collect_render_chunk_imports(ctx.chunk, ctx.link_output, ctx.chunk_graph);

  let mut s = String::new();
  let mut externals = ExternalModules::new();
  render_import_stmts.iter().for_each(|stmt| {
    match &stmt.specifiers {
      RenderImportDeclarationSpecifier::ImportSpecifier(specifiers) => {
        // Empty specifiers can be ignored in IIFE.
        if !specifiers.is_empty() {
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
          externals.push(stmt.path.to_string(), specifiers.is_empty());
        }
      }
      RenderImportDeclarationSpecifier::ImportStarSpecifier(alias) => {
        s.push_str(&format!("const {alias} = {};\n", legitimize_identifier_name(&stmt.path)));
        externals.push(stmt.path.to_string(), false);
      }
    }
  });

  (s, externals)
}

/// Metadata for the factory function. e.g. In iife, it should be `var ... = (function(exports, ...){`, and the caller should be `})(...);`.
fn render_factory(
  ctx: &mut GenerateContext<'_>,
  export_mode: &OutputExports,
  has_export: bool,
  args: &ExternalModules,
) -> DiagnosableResult<(String, String)> {
  match ctx.options.format {
    OutputFormat::Iife => render_iife_factory(ctx, export_mode, has_export, args),
    _ => unreachable!(),
  }
}
