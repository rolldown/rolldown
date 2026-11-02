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
use crate::utils::chunk::render_chunk_exports::{
  get_chunk_export_names_with_ctx, render_wrapped_entry_chunk,
};
use crate::{
  types::generator::GenerateContext, utils::chunk::render_chunk_exports::render_chunk_exports,
};
use rolldown_common::{AddonRenderContext, ExternalModule, OutputExports};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_utils::{concat_string, ecmascript::escape_non_ascii};

use super::{
  utils::{render_chunk_directives, render_chunk_external_imports, render_factory_parameters},
  wrapped::WrappedPlan,
};

pub async fn prepare_iife(
  ctx: &GenerateContext<'_>,
  addon_render_context: AddonRenderContext<'_>,
  warnings: &mut Vec<BuildDiagnostic>,
) -> BuildResult<WrappedPlan> {
  let mut prefix = Vec::new();
  let mut suffix = Vec::new();
  let capture_ast = ctx.defer_ast
    && !crate::stages::generate_stage::chunk_ast_dce::complete_program(
      ctx.options,
      ctx.plugin_driver,
    );
  let mut ast_tail = Vec::new();
  let AddonRenderContext { hashbang, banner, intro, outro, footer, directives } =
    addon_render_context;
  if let Some(hashbang) = hashbang {
    prefix.push(hashbang.to_string());
  }

  if let Some(banner) = banner {
    prefix.push(banner.to_string());
  }

  if !directives.is_empty() {
    let rendered_chunk_directives = render_chunk_directives(directives.iter());
    if !rendered_chunk_directives.is_empty() {
      prefix.push(rendered_chunk_directives);
    }
  }

  // iife wrapper start

  // Use pre-computed output_exports from the chunk
  let export_mode = ctx.chunk.output_exports;

  let export_names = get_chunk_export_names_with_ctx(ctx);
  let has_exports = !export_names.is_empty();
  let has_default_export = export_names.iter().any(|name| name.as_str() == "default");

  let entry_module = ctx
    .chunk
    .entry_module(&ctx.link_output.module_table)
    .expect("iife format only have entry chunk");

  let named_exports = matches!(&export_mode, OutputExports::Named);

  // It is similar to CJS.
  let (import_code, externals) = render_chunk_external_imports(ctx);
  // For IIFE, we only include externals with used exports in factory parameters/arguments
  // because we don't have a way to trigger side effects.
  let externals: Vec<_> = externals
    .iter()
    .filter(|e| e.is_used())
    .map(super::utils::ExternalImportKind::module)
    .collect();

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

  prefix.push(concat_string!(
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

  if let Some(intro) = intro {
    prefix.push(intro.to_string());
  }

  if named_exports && entry_module.exports_kind.is_esm() {
    // Symbol.toStringTag should only be added to module facades (chunks that represent a specific module)
    if let Some(marker) = render_namespace_markers(
      ctx.options.es_module,
      has_default_export,
      &ctx.options.generated_code,
      ctx.chunk.is_entry_point(),
    ) {
      prefix.push(marker.to_string());
    }
  }

  if let Some(source) = render_wrapped_entry_chunk(ctx, Some(&export_mode)) {
    if capture_ast
      && (matches!(export_mode, OutputExports::Named)
        || !matches!(
          ctx.link_output.metas[entry_module.idx].wrap_kind(),
          rolldown_common::WrapKind::Cjs
        ))
    {
      ast_tail.push(source.clone());
    }
    suffix.push(source);
  }

  // iife exports
  if let Some(exports) = render_chunk_exports(ctx, Some(&export_mode)) {
    if capture_ast && !matches!(export_mode, OutputExports::Default) {
      ast_tail.push(exports.clone());
    }
    suffix.push(exports);
  }

  if let Some(outro) = outro {
    suffix.push(outro.to_string());
  }

  if named_exports && has_exports && !ctx.options.extend {
    // We need to add `return exports;` here only if using `named`, because the default value is returned when using `default` in `render_chunk_exports`.
    suffix.push("return exports;".to_string());
  }

  // iife wrapper end
  let (factory_arguments, globals) =
    render_iife_factory_arguments(warnings, ctx, &externals, exports_prefix, capture_ast).await?;
  suffix.push(concat_string!("})(", factory_arguments, ");"));

  if let Some(footer) = footer {
    suffix.push(footer.to_string());
  }

  let ast = capture_ast.then(|| {
    let mut parameters = Vec::with_capacity(externals.len() + 1);
    if exports_prefix.is_some() {
      parameters.push("exports".into());
    }
    parameters.extend(externals.iter().map(|external| {
      ctx
        .link_output
        .symbol_db
        .canonical_name_for_or_original(external.namespace_ref, &ctx.chunk.canonical_names)
        .to_string()
    }));
    super::wrapper_ast::WrapperAstPlan {
      kind: super::wrapper_ast::WrapperKind::Iife,
      name: ctx.options.name.clone(),
      extend: ctx.options.extend,
      has_exports,
      named_exports,
      parameters,
      globals,
      es_module: named_exports
        && entry_module.exports_kind.is_esm()
        && crate::utils::chunk::namespace_marker::determine_es_module(
          ctx.options.es_module,
          has_default_export,
        ),
      to_string_tag: named_exports
        && entry_module.exports_kind.is_esm()
        && ctx.options.generated_code.symbols
        && ctx.chunk.is_entry_point(),
      tail: ast_tail.join("\n"),
      outer_prefix: banner.unwrap_or_default().to_string(),
      outer_suffix: footer.unwrap_or_default().to_string(),
      hashbang: hashbang.map(|value| value.trim_start_matches("#!").to_string()),
      directives: directives.iter().map(|value| (*value).to_string()).collect(),
      factory_return: super::wrapper_ast::WrapperAstPlan::factory_return(ctx),
    }
  });
  Ok(WrappedPlan { prefix, imports: import_code, suffix, runtime_before_imports: true, ast })
}

async fn render_iife_factory_arguments(
  warnings: &mut Vec<BuildDiagnostic>,
  ctx: &GenerateContext<'_>,
  externals: &[&ExternalModule],
  exports_prefix: Option<&str>,
  capture_ast: bool,
) -> BuildResult<(String, Vec<String>)> {
  let mut resolved_globals = Vec::with_capacity(if capture_ast { externals.len() } else { 0 });
  let mut factory_arguments = if let Some(exports_prefix) = exports_prefix {
    vec![exports_prefix.to_string()]
  } else {
    vec![]
  };
  let globals = &ctx.options.globals;
  for external in externals {
    let global = globals.call(external.id.as_str()).await?;
    let target = match global {
      Some(global_name) => global_name,
      None => {
        warnings.push(
          BuildDiagnostic::missing_global_name(
            external.id.to_string(),
            external.name.clone(),
            external.identifier_name.clone(),
          )
          .with_severity_warning(),
        );
        external.identifier_name.to_string()
      }
    };
    factory_arguments.push(escape_non_ascii(&target, ctx.options.charset.is_ascii()).into_owned());
    if capture_ast {
      resolved_globals.push(target);
    }
  }
  Ok((factory_arguments.join(", "), resolved_globals))
}
