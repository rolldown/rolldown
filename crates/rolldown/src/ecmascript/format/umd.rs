use arcstr::ArcStr;
use rolldown_common::{ChunkKind, OutputExports};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::ecmascript::legitimize_identifier_name;

use crate::{
  ecmascript::{
    ecma_generator::RenderedModuleSources, format::utils::namespace::generate_namespace_definition,
  },
  types::generator::GenerateContext,
  utils::chunk::{
    collect_render_chunk_imports::ExternalRenderImportStmt,
    determine_export_mode::determine_export_mode,
    determine_use_strict::determine_use_strict,
    namespace_marker::render_namespace_markers,
    render_chunk_exports::{get_export_items, render_chunk_exports},
  },
};

use super::utils::{
  namespace::render_property_access, render_chunk_external_imports, render_factory_parameters,
};

pub fn render_umd<'code>(
  ctx: &mut GenerateContext<'_>,
  module_sources: &'code RenderedModuleSources,
  banner: Option<&'code str>,
  footer: Option<&'code str>,
  intro: Option<&'code str>,
  outro: Option<&'code str>,
) -> BuildResult<SourceJoiner<'code>> {
  let mut source_joiner = SourceJoiner::default();

  if let Some(banner) = banner {
    source_joiner.append_source(banner);
  }

  // umd wrapper start

  // Analyze the export information of the chunk.
  let export_items = get_export_items(ctx.chunk, ctx.link_output);
  let has_exports = !export_items.is_empty();
  let has_default_export = export_items.iter().any(|(name, _)| name.as_str() == "default");

  let entry_module = match ctx.chunk.kind {
    ChunkKind::EntryPoint { module, .. } => {
      &ctx.link_output.module_table.modules[module].as_normal().expect("should be normal module")
    }
    ChunkKind::Common => unreachable!("umd should be entry point chunk"),
  };

  // We need to transform the `OutputExports::Auto` to suitable `OutputExports`.
  let export_mode = determine_export_mode(ctx, entry_module, &export_items)?;

  let named_exports = matches!(&export_mode, OutputExports::Named);

  // It is similar to CJS.
  let (import_code, externals) = render_chunk_external_imports(ctx);

  // The function argument and the external imports are passed as arguments to the wrapper function.
  let need_global = has_exports || named_exports || !externals.is_empty();
  let wrapper_parameters = if need_global { "global, factory" } else { "factory" };
  let amd_dependencies = render_amd_dependencies(&externals, has_exports && named_exports);
  let global_argument = if need_global { "this, " } else { "" };
  let factory_parameters = render_factory_parameters(ctx, &externals, has_exports && named_exports);
  let cjs_intro = if need_global {
    let cjs_export = if has_exports && !named_exports { "module.exports = " } else { "" };
    let cjs_dependencies = render_cjs_dependencies(&externals, has_exports && named_exports);
    format!("typeof exports === 'object' && typeof module !== 'undefined' ? {cjs_export} factory({cjs_dependencies}) :",)
  } else {
    String::new()
  };
  let iife_start = if need_global {
    "(global = typeof globalThis !== 'undefined' ? globalThis : global || self, "
  } else {
    ""
  };
  let iife_end = if need_global { ")" } else { "" };
  let iife_export = render_iife_export(ctx, &externals, has_exports, named_exports)?;
  source_joiner.append_source(format!(
    "(function({wrapper_parameters}) {{
  {cjs_intro}
  typeof define === 'function' && define.amd ? define([{amd_dependencies}], factory) :
  {iife_start}{iife_export}{iife_end};
}})({global_argument}function({factory_parameters}) {{",
  ));

  if determine_use_strict(ctx) {
    source_joiner.append_source("\"use strict\";");
  }

  if let Some(intro) = intro {
    source_joiner.append_source(intro);
  }

  if named_exports {
    if let Some(marker) =
      render_namespace_markers(&ctx.options.es_module, has_default_export, false)
    {
      source_joiner.append_source(marker.to_string());
    }
  }

  source_joiner.append_source(import_code);

  // chunk content
  // TODO indent chunk content
  module_sources.iter().for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources {
        source_joiner.append_source(source);
      }
    }
  });

  //  exports
  if let Some(exports) = render_chunk_exports(ctx, Some(&export_mode)) {
    source_joiner.append_source(exports);
  }

  if let Some(outro) = outro {
    source_joiner.append_source(outro);
  }

  // umd wrapper end
  source_joiner.append_source("});");

  if let Some(footer) = footer {
    source_joiner.append_source(footer);
  }

  Ok(source_joiner)
}

fn render_amd_dependencies(externals: &[ExternalRenderImportStmt], has_exports: bool) -> String {
  let mut dependencies = Vec::with_capacity(externals.len());
  if has_exports {
    dependencies.reserve(1);
    dependencies.push("exports".to_string());
  }
  externals.iter().for_each(|external| {
    dependencies.push(format!("'{}'", external.path.as_str()));
  });
  dependencies.join(", ")
}

fn render_cjs_dependencies(externals: &[ExternalRenderImportStmt], has_exports: bool) -> String {
  let mut dependencies = Vec::with_capacity(externals.len());
  if has_exports {
    dependencies.reserve(1);
    dependencies.push("exports".to_string());
  }
  externals.iter().for_each(|external| {
    dependencies.push(format!("require('{}')", external.path.as_str()));
  });
  dependencies.join(", ")
}

fn render_iife_export(
  ctx: &mut GenerateContext<'_>,
  externals: &[ExternalRenderImportStmt],
  has_exports: bool,
  named_exports: bool,
) -> BuildResult<String> {
  if has_exports && ctx.options.name.as_ref().map_or(true, String::is_empty) {
    return Err(vec![BuildDiagnostic::missing_name_option_for_umd_export()].into());
  }
  let mut dependencies = Vec::with_capacity(externals.len());
  externals.iter().for_each(|external| {
    if let Some(global) = ctx.options.globals.get(external.path.as_str()) {
      dependencies.push(format!(
        "global{}",
        global.split('.').map(render_property_access).collect::<String>()
      ));
    } else {
      let target = legitimize_identifier_name(external.path.as_str()).to_string();
      ctx.warnings.push(
        BuildDiagnostic::missing_global_name(external.path.clone(), ArcStr::from(&target))
          .with_severity_warning(),
      );
      dependencies.push(format!("global{}", render_property_access(&target)));
    }
  });
  let deps = dependencies.join(",");
  if has_exports {
    let (stmt, namespace) = generate_namespace_definition(
      ctx.options.name.as_ref().expect("should have name"),
      "global",
      ",",
    );
    if named_exports {
      Ok(format!(
        "factory(({stmt}{namespace} = {}){})",
        if ctx.options.extend { format!("{namespace} || {{}}") } else { "{}".to_string() },
        if dependencies.is_empty() { String::new() } else { format!(", {deps}") }
      ))
    } else {
      Ok(format!("({stmt}{namespace} = factory({deps}))"))
    }
  } else {
    Ok(format!("factory({deps})"))
  }
}
