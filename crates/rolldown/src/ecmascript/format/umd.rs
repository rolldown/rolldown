use arcstr::ArcStr;
use rolldown_common::{ChunkKind, ExternalModule, OutputExports, WrapKind};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::{concat_string, ecmascript::legitimize_identifier_name};

use crate::{
  ecmascript::{
    ecma_generator::RenderedModuleSources, format::utils::namespace::generate_namespace_definition,
  },
  types::generator::GenerateContext,
  utils::chunk::{
    determine_export_mode::determine_export_mode,
    determine_use_strict::determine_use_strict,
    namespace_marker::render_namespace_markers,
    render_chunk_exports::{get_export_items, render_chunk_exports},
  },
};

use super::utils::{
  namespace::render_property_access, render_chunk_external_imports, render_factory_parameters,
};

#[expect(clippy::too_many_lines)]
pub async fn render_umd<'code>(
  ctx: &GenerateContext<'_>,
  banner: Option<&'code str>,
  intro: Option<&'code str>,
  outro: Option<&'code str>,
  footer: Option<&'code str>,
  module_sources: &'code RenderedModuleSources,
  warnings: &mut Vec<BuildDiagnostic>,
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
  let export_mode = determine_export_mode(warnings, ctx, entry_module, &export_items)?;

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
  let iife_export =
    render_iife_export(warnings, ctx, &externals, has_exports, named_exports).await?;
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
    if let Some(marker) = render_namespace_markers(ctx.options.es_module, has_default_export, false)
    {
      source_joiner.append_source(marker.to_string());
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
  // TODO indent chunk content
  module_sources_peekable.for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources.as_ref() {
        source_joiner.append_source(source);
      }
    }
  });

  if let ChunkKind::EntryPoint { module: entry_id, .. } = ctx.chunk.kind {
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

        // require_xxx();
        // FIXME: should set the umd's exports with `require_xxx()`
        source_joiner.append_source(concat_string!(wrapper_ref_name, "();\n"));
      }
      WrapKind::None => {}
    }
  }

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

fn render_amd_dependencies(externals: &[&ExternalModule], has_exports: bool) -> String {
  let mut dependencies = Vec::with_capacity(externals.len());
  if has_exports {
    dependencies.reserve(1);
    dependencies.push("exports".to_string());
  }
  externals.iter().for_each(|external| {
    dependencies.push(format!("'{}'", external.name.as_str()));
  });
  dependencies.join(", ")
}

fn render_cjs_dependencies(externals: &[&ExternalModule], has_exports: bool) -> String {
  let mut dependencies = Vec::with_capacity(externals.len());
  if has_exports {
    dependencies.reserve(1);
    dependencies.push("exports".to_string());
  }
  externals.iter().for_each(|external| {
    dependencies.push(format!("require('{}')", external.name.as_str()));
  });
  dependencies.join(", ")
}

async fn render_iife_export(
  warnings: &mut Vec<BuildDiagnostic>,
  ctx: &GenerateContext<'_>,
  externals: &[&ExternalModule],
  has_exports: bool,
  named_exports: bool,
) -> BuildResult<String> {
  if has_exports && ctx.options.name.as_ref().map_or(true, String::is_empty) {
    return Err(vec![BuildDiagnostic::missing_name_option_for_umd_export()].into());
  }
  let mut dependencies = Vec::with_capacity(externals.len());

  for external in externals {
    let global = ctx.options.globals.call(external.name.as_str()).await;
    let target = match &global {
      Some(global_name) => global_name.split('.').map(render_property_access).collect::<String>(),
      None => {
        let target = legitimize_identifier_name(external.name.as_str()).to_string();
        warnings.push(
          BuildDiagnostic::missing_global_name(external.name.clone(), ArcStr::from(&target))
            .with_severity_warning(),
        );
        render_property_access(&target)
      }
    };
    dependencies.push(format!("global{target}"));
  }

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
