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
use rolldown_common::{ExternalModule, OutputExports, WrapKind};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::{concat_string, ecmascript::legitimize_identifier_name};

use super::utils::{render_chunk_external_imports, render_factory_parameters};

/// The main function for rendering the IIFE format chunks.
#[expect(clippy::too_many_arguments, clippy::too_many_lines)]
pub async fn render_iife<'code>(
  ctx: &GenerateContext<'_>,
  hashbang: Option<&'code str>,
  banner: Option<&'code str>,
  intro: Option<&'code str>,
  outro: Option<&'code str>,
  footer: Option<&'code str>,
  module_sources: &'code RenderedModuleSources,
  warnings: &mut Vec<BuildDiagnostic>,
) -> BuildResult<SourceJoiner<'code>> {
  let mut source_joiner = SourceJoiner::default();

  if let Some(hashbang) = hashbang {
    source_joiner.append_source(hashbang);
  }

  if let Some(banner) = banner {
    source_joiner.append_source(banner);
  }

  // iife wrapper start

  // Analyze the export information of the chunk.
  let export_items = get_export_items(ctx.chunk, ctx.link_output);
  let has_exports = !export_items.is_empty();
  let has_default_export = export_items.iter().any(|(name, _)| name.as_str() == "default");

  let entry_module = ctx
    .chunk
    .entry_module(&ctx.link_output.module_table)
    .expect("iife format only have entry chunk");

  // We need to transform the `OutputExports::Auto` to suitable `OutputExports`.
  let export_mode = determine_export_mode(warnings, ctx, entry_module, &export_items)?;

  let named_exports = matches!(&export_mode, OutputExports::Named);

  // It is similar to CJS.
  let (import_code, externals) = render_chunk_external_imports(ctx);

  // Generate the identifier for the IIFE wrapper function.
  // You can refer to the function for more details.
  let (definition, assignment) = generate_identifier(warnings, ctx, export_mode)?;

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

  source_joiner.append_source(concat_string!(
    definition,
    if (ctx.options.extend && named_exports) || !has_exports || assignment.is_empty() {
      // If facing following situations, there shouldn't an assignment for the wrapper function:
      // - Using `output.extend` and named export.
      // - No export.
      // - the `assignment` is empty.
      String::new()
    } else {
      concat_string!(assignment, " = ")
    },
    "(function(",
    factory_parameters,
    ") {\n"
  ));

  if determine_use_strict(ctx) {
    source_joiner.append_source("\"use strict\";");
  }

  if let Some(intro) = intro {
    source_joiner.append_source(intro);
  }

  if named_exports {
    if let Some(marker) = render_namespace_markers(ctx.options.es_module, has_default_export, false)
    {
      source_joiner.append_source(marker);
    }
  }

  let mut module_sources_peekable = module_sources.iter().peekable();
  match module_sources_peekable.peek() {
    Some((id, _, _)) if *id == ctx.link_output.runtime.id() => {
      if let (_, _module_id, Some(emitted_sources)) =
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
  module_sources_peekable.for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources.as_ref() {
        source_joiner.append_source(source);
      }
    }
  });

  if let Some(entry_id) = ctx.chunk.entry_module_idx() {
    let entry_meta = &ctx.link_output.metas[entry_id];
    match entry_meta.wrap_kind {
      WrapKind::Esm => {
        let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();
        // init_xxx
        let wrapper_ref_name = ctx.finalized_string_pattern_for_symbol_ref(
          *wrapper_ref,
          ctx.chunk_idx,
          &ctx.chunk.canonical_names,
        );
        ctx.link_output.symbol_db.canonical_name_for(*wrapper_ref, &ctx.chunk.canonical_names);
        source_joiner.append_source(concat_string!(wrapper_ref_name, "();"));
      }
      WrapKind::Cjs => {
        let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();

        // require_xxx
        let wrapper_ref_name = ctx.finalized_string_pattern_for_symbol_ref(
          *wrapper_ref,
          ctx.chunk_idx,
          &ctx.chunk.canonical_names,
        );

        // return require_xxx();
        source_joiner.append_source(concat_string!("return ", wrapper_ref_name, "();\n"));
      }
      WrapKind::None => {}
    }
  }

  // iife exports
  if let Some(exports) = render_chunk_exports(ctx, Some(&export_mode)) {
    source_joiner.append_source(exports);
  }

  if let Some(outro) = outro {
    source_joiner.append_source(outro);
  }

  if named_exports && has_exports && !ctx.options.extend {
    // We need to add `return exports;` here only if using `named`, because the default value is returned when using `default` in `render_chunk_exports`.
    source_joiner.append_source("return exports;");
  }

  // iife wrapper end
  let factory_arguments =
    render_iife_factory_arguments(warnings, ctx, &externals, exports_prefix).await;
  source_joiner.append_source(concat_string!("})(", factory_arguments, ");"));

  if let Some(footer) = footer {
    source_joiner.append_source(footer);
  }

  Ok(source_joiner)
}

async fn render_iife_factory_arguments(
  warnings: &mut Vec<BuildDiagnostic>,
  ctx: &GenerateContext<'_>,
  externals: &[&ExternalModule],
  exports_prefix: Option<&str>,
) -> String {
  let mut factory_arguments = if let Some(exports_prefix) = exports_prefix {
    vec![exports_prefix.to_string()]
  } else {
    vec![]
  };
  let globals = &ctx.options.globals;
  for external in externals {
    let global = globals.call(external.name.as_str()).await;
    let target = match &global {
      Some(global_name) => legitimize_identifier_name(global_name).to_string(),
      None => {
        let target = legitimize_identifier_name(&external.name).to_string();
        warnings.push(
          BuildDiagnostic::missing_global_name(external.name.clone(), ArcStr::from(&target))
            .with_severity_warning(),
        );
        target
      }
    };
    factory_arguments.push(target);
  }
  factory_arguments.join(", ")
}
