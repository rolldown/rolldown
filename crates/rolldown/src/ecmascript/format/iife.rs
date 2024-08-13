//! This is the render function for IIFE format.
//! It wraps the chunk content in an IIFE.
//!
//! 1. Render the banner if it exists.
//! 2. Start the wrapper function, and determine the export mode (from auto or manual exports).
//! 3. Render the imports and modify the arguments of the wrapper function.
//!    Including:
//!       - Render the arguments including the function arguments and the external imports,
//!         according to the `output.globals`, or if you are using named export,
//!         the function will pass the `exports` argument with default `{}` as the first argument.
//!       - Generate the statement for a namespace level-by-level and define the IIFE wrapper
//!         function name if `output.extends` is false, or the export mode isn't `named`.
//!
//!    Note that in IIFE, the external imports are directly assigned to the global variables.
//!    And in the wrapper function, the global variables are passed as arguments.
//! 4. Check if the chunk is suitable for strict mode, and add `"use strict";` if necessary.
//! 5. Render the intro if it exists.
//! 6. Render the chunk content.
//! 7. Render the exports if it exists. If you are using named export, it will modify the `exports` object.
//!    If you are using default export, it will return the default value.
//! 8. Render the outro if it exists.
//! 9. The wrapper function ends with `})({output_args});` if `invoke` is true, otherwise, it ends with `})`. (for UMD capability)
//! 10. Render the footer if it exists.

use crate::ecmascript::format::utils::namespace::generate_identifier;
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
/// The main function for rendering the IIFE format chunks.
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

  // Analyze the export information of the chunk.
  let export_items = get_export_items(ctx.chunk, ctx.link_output);
  let has_exports = !export_items.is_empty();
  let has_default_export = export_items.iter().any(|(name, _)| name.as_str() == "default");

  let entry_module = match ctx.chunk.kind {
    ChunkKind::EntryPoint { module, .. } => {
      &ctx.link_output.module_table.modules[module].as_ecma().expect("should be ecma module")
    }
    ChunkKind::Common => unreachable!("iife should be entry point chunk"),
  };

  // We need to transform the `OutputExports::Auto` to suitable `OutputExports`.
  let export_mode = determine_export_mode(ctx, entry_module, &export_items)?;

  let named_exports = matches!(&export_mode, OutputExports::Named);

  // It is similar to CJS.
  let (import_code, externals) = render_iife_chunk_imports(ctx);

  // Generate the identifier for the IIFE wrapper function.
  // You can refer to the function for more details.
  let (definition, assignment) = generate_identifier(ctx, &export_mode)?;

  // The function argument and the external imports are passed as arguments to the wrapper function.
  let (input_args, output_args) = render_iife_arguments(
    ctx,
    &externals,
    if has_exports && named_exports {
      if ctx.options.extend {
        // If using `output.extend`, the first caller argument should be `name = name || {}`,
        // then the result will be assigned to `name`.
        Some(assignment.as_str())
      } else {
        // If not using `output.extend`, the first caller argument should be `{}`,
        // then the result will be assigned to `exports`.
        Some("{}")
      }
    } else {
      // If there is no export or not using named export,
      // there shouldn't be an argument shouldn't be related to the export.
      None
    },
  );

  concat_source.add_source(Box::new(RawSource::new(format!(
    "{definition}{}(function({input_args}) {{\n",
    if (ctx.options.extend && named_exports) || !has_exports || assignment.is_empty() {
      // If facing following situations, there shouldn't an assignment for the wrapper function:
      // - Using `output.extend` and named export.
      // - No export.
      // - the `assignment` is empty.
      String::new()
    } else {
      format!("{assignment} = ")
    }
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

  if named_exports && has_exports && !ctx.options.extend {
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

/// Handling external imports needs to modify the arguments of the wrapper function.
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

/// Rendering the arguments of the wrapper function, including the function arguments and calling arguments.
/// - If `output.exports` is `named`, the first argument is `exports`.
///    - If you are using `extend: true`, the inputted argument will be extended;
///    - If you aren't using it, the wrapper function will return the `exports` object as the result.
///
///    If `output.exports` is `default`, there is no need to pass the `exports` argument.
///    The return value of the wrapper function will be the default export value.
/// - The rest of the arguments are the external imports, which are directly assigned to the global variables.
///    - If the global variable is not defined in `output.globals`,
///      you will be warned and rolldown will use the defined name after legitimizing it.
///    - If the global variable is defined in `output.globals`, the global variable will be used directly (after legitimizing it).
fn render_iife_arguments(
  ctx: &mut GenerateContext<'_>,
  externals: &[String],
  exports_prefix: Option<&str>,
) -> (String, String) {
  let mut input_args = if exports_prefix.is_some() { vec!["exports".to_string()] } else { vec![] };
  let mut output_args = if let Some(exports_prefix) = exports_prefix {
    vec![exports_prefix.to_string()]
  } else {
    vec![]
  };
  let globals = &ctx.options.globals;
  externals.iter().for_each(|external| {
    // TODO deconflict input args
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
