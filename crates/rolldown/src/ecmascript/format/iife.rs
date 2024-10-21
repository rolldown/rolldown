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
use crate::utils::chunk::collect_render_chunk_imports::ExternalRenderImportStmt;
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

use super::utils::{render_chunk_external_imports, render_factory_parameters};

/// The main function for rendering the IIFE format chunks.
pub fn render_iife(
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

  // Analyze the export information of the chunk.
  let export_items = get_export_items(ctx.chunk, ctx.link_output);
  let has_exports = !export_items.is_empty();
  let has_default_export = export_items.iter().any(|(name, _)| name.as_str() == "default");

  let entry_module = match ctx.chunk.kind {
    ChunkKind::EntryPoint { module, .. } => {
      &ctx.link_output.module_table.modules[module].as_normal().expect("should be normal module")
    }
    ChunkKind::Common => unreachable!("iife should be entry point chunk"),
  };

  // We need to transform the `OutputExports::Auto` to suitable `OutputExports`.
  let export_mode = determine_export_mode(ctx, entry_module, &export_items)?;

  let named_exports = matches!(&export_mode, OutputExports::Named);

  // It is similar to CJS.
  let (import_code, externals) = render_chunk_external_imports(ctx);

  // Generate the identifier for the IIFE wrapper function.
  // You can refer to the function for more details.
  let (definition, assignment) = generate_identifier(ctx, &export_mode)?;

  let exports_prefix = if has_exports && named_exports {
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
  };
  // The function argument and the external imports are passed as arguments to the wrapper function.
  let factory_parameters = render_factory_parameters(ctx, &externals, exports_prefix.is_some());

  concat_source.add_source(Box::new(RawSource::new(format!(
    "{definition}{}(function({factory_parameters}) {{\n",
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
  let factory_arguments = render_iife_factory_arguments(ctx, &externals, exports_prefix);
  concat_source.add_source(Box::new(RawSource::new(format!("}})({factory_arguments});"))));

  if let Some(footer) = footer {
    concat_source.add_source(Box::new(RawSource::new(footer)));
  }

  Ok(concat_source)
}

fn render_iife_factory_arguments(
  ctx: &mut GenerateContext<'_>,
  externals: &[ExternalRenderImportStmt],
  exports_prefix: Option<&str>,
) -> String {
  let mut factory_arguments = if let Some(exports_prefix) = exports_prefix {
    vec![exports_prefix.to_string()]
  } else {
    vec![]
  };
  let globals = &ctx.options.globals;
  externals.iter().for_each(|external| {
    if let Some(global) = globals.get(external.path.as_str()) {
      factory_arguments.push(legitimize_identifier_name(global).to_string());
    } else {
      let target = legitimize_identifier_name(external.path.as_str()).to_string();
      ctx.warnings.push(
        BuildDiagnostic::missing_global_name(external.path.clone(), ArcStr::from(&target))
          .with_severity_warning(),
      );
      factory_arguments.push(target);
    }
  });
  factory_arguments.join(", ")
}
