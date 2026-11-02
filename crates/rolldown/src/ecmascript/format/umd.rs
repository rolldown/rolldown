use rolldown_common::{AddonRenderContext, ExternalModule, OutputExports};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_utils::{
  concat_string,
  ecmascript::{property_access_str_with_ascii, to_ecma_string_literal},
};

use crate::{
  ecmascript::format::utils::namespace::generate_namespace_definition,
  types::generator::GenerateContext,
  utils::chunk::{
    namespace_marker::render_namespace_markers,
    render_chunk_exports::{
      get_chunk_export_names_with_ctx, render_chunk_exports, render_wrapped_entry_chunk,
    },
  },
};

use super::{
  utils::{render_chunk_directives, render_chunk_external_imports, render_factory_parameters},
  wrapped::WrappedPlan,
};

pub async fn prepare_umd(
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
  let AddonRenderContext { banner, intro, outro, footer, directives, .. } = addon_render_context;
  if let Some(banner) = banner {
    prefix.push(banner.to_string());
  }

  if !directives.is_empty() {
    let rendered_chunk_directives = render_chunk_directives(directives.iter());
    if !rendered_chunk_directives.is_empty() {
      prefix.push(rendered_chunk_directives);
    }
  }

  // umd wrapper start

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
  let externals: Vec<_> = externals.iter().map(super::utils::ExternalImportKind::module).collect();

  // The function argument and the external imports are passed as arguments to the wrapper function.
  let need_global = has_exports || named_exports || !externals.is_empty();
  let wrapper_parameters = if need_global { "global, factory" } else { "factory" };
  let amd_dependencies = render_amd_dependencies(ctx, &externals, has_exports && named_exports);
  let global_argument = if need_global { "this, " } else { "" };
  let factory_parameters = render_factory_parameters(ctx, &externals, has_exports && named_exports);
  let cjs_intro = if need_global {
    let cjs_export = if has_exports && !named_exports { "module.exports = " } else { "" };
    let cjs_dependencies = render_cjs_dependencies(ctx, &externals, has_exports && named_exports);
    format!(
      "typeof exports === 'object' && typeof module !== 'undefined' ? {cjs_export} factory({cjs_dependencies}) :",
    )
  } else {
    String::new()
  };
  let iife_start = if need_global {
    "(global = typeof globalThis !== 'undefined' ? globalThis : global || self, "
  } else {
    ""
  };
  let iife_end = if need_global { ")" } else { "" };
  let (iife_export, globals) =
    render_iife_export(warnings, ctx, &externals, has_exports, named_exports, capture_ast).await?;
  prefix.push(format!(
    "(function({wrapper_parameters}) {{
  {cjs_intro}
  typeof define === 'function' && define.amd ? define([{amd_dependencies}], factory) :
  {iife_start}{iife_export}{iife_end};
}})({global_argument}function({factory_parameters}) {{",
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

  //  exports
  if let Some(exports) = render_chunk_exports(ctx, Some(&export_mode)) {
    if capture_ast && !matches!(export_mode, OutputExports::Default) {
      ast_tail.push(exports.clone());
    }
    suffix.push(exports);
  }

  if let Some(outro) = outro {
    suffix.push(outro.to_string());
  }

  // umd wrapper end
  suffix.push("});".to_string());

  if let Some(footer) = footer {
    suffix.push(footer.to_string());
  }

  let ast = capture_ast.then(|| {
    let mut parameters = Vec::with_capacity(externals.len() + 1);
    if has_exports && named_exports {
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
      kind: super::wrapper_ast::WrapperKind::Umd {
        paths: externals
          .iter()
          .map(|external| external.get_import_path(ctx.chunk, ctx.resolved_paths).to_string())
          .collect(),
      },
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
      hashbang: None,
      directives: directives.iter().map(|value| (*value).to_string()).collect(),
      factory_return: super::wrapper_ast::WrapperAstPlan::factory_return(ctx),
    }
  });
  Ok(WrappedPlan { prefix, imports: import_code, suffix, runtime_before_imports: true, ast })
}

fn render_amd_dependencies(
  ctx: &GenerateContext<'_>,
  externals: &[&ExternalModule],
  has_exports: bool,
) -> String {
  let mut dependencies = Vec::with_capacity(externals.len());
  if has_exports {
    dependencies.reserve(1);
    dependencies.push("'exports'".to_string());
  }
  externals.iter().for_each(|external| {
    dependencies.push(to_ecma_string_literal(
      &external.get_import_path(ctx.chunk, ctx.resolved_paths),
      ctx.options.charset.is_ascii(),
    ));
  });
  dependencies.join(", ")
}

fn render_cjs_dependencies(
  ctx: &GenerateContext<'_>,
  externals: &[&ExternalModule],
  has_exports: bool,
) -> String {
  let mut dependencies = Vec::with_capacity(externals.len());
  if has_exports {
    dependencies.reserve(1);
    dependencies.push("exports".to_string());
  }
  externals.iter().for_each(|external| {
    let import_path = to_ecma_string_literal(
      &external.get_import_path(ctx.chunk, ctx.resolved_paths),
      ctx.options.charset.is_ascii(),
    );
    dependencies.push(concat_string!("require(", import_path, ")"));
  });
  dependencies.join(", ")
}

async fn render_iife_export(
  warnings: &mut Vec<BuildDiagnostic>,
  ctx: &GenerateContext<'_>,
  externals: &[&ExternalModule],
  has_exports: bool,
  named_exports: bool,
  capture_ast: bool,
) -> BuildResult<(String, Vec<String>)> {
  if has_exports && ctx.options.name.as_ref().is_none_or(String::is_empty) {
    return Err(vec![BuildDiagnostic::missing_name_option_for_iife_export(true)].into());
  }
  let mut dependencies = Vec::with_capacity(externals.len());
  let mut resolved_globals = Vec::with_capacity(if capture_ast { externals.len() } else { 0 });

  for external in externals {
    let global = ctx.options.globals.call(external.id.as_str()).await?;
    let target = match &global {
      Some(global_name) => global_name
        .split('.')
        .map(|part| property_access_str_with_ascii("", part, ctx.options.charset.is_ascii()))
        .collect::<String>(),
      None => {
        warnings.push(
          BuildDiagnostic::missing_global_name(
            external.id.to_string(),
            external.name.clone(),
            external.identifier_name.clone(),
          )
          .with_severity_warning(),
        );
        property_access_str_with_ascii(
          "",
          &external.identifier_name,
          ctx.options.charset.is_ascii(),
        )
      }
    };
    dependencies.push(format!("global{target}"));
    if capture_ast {
      resolved_globals.push(global.unwrap_or_else(|| external.identifier_name.to_string()));
    }
  }

  let deps = dependencies.join(",");
  if has_exports {
    let (stmt, namespace) = generate_namespace_definition(
      ctx.options.name.as_ref().expect("should have name"),
      "global",
      ",",
      ctx.options.charset.is_ascii(),
    );
    if named_exports {
      Ok((
        format!(
          "factory(({stmt}{namespace} = {}){})",
          if ctx.options.extend { format!("{namespace} || {{}}") } else { "{}".to_string() },
          if dependencies.is_empty() { String::new() } else { format!(", {deps}") }
        ),
        resolved_globals,
      ))
    } else {
      Ok((format!("({stmt}{namespace} = factory({deps}))"), resolved_globals))
    }
  } else {
    Ok((format!("factory({deps})"), resolved_globals))
  }
}
