use rolldown_common::{ChunkKind, OutputExports};
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_sourcemap::{ConcatSource, RawSource};

use crate::{
  ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::{
    collect_render_chunk_imports::ExternalRenderImportStmt,
    determine_export_mode::determine_export_mode,
    determine_use_strict::determine_use_strict,
    namespace_marker::render_namespace_markers,
    render_chunk_exports::{get_export_items, render_chunk_exports},
  },
};

use super::utils::{render_chunk_external_imports, render_factory_parameters};

pub fn render_umd(
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
  let factory_parameters = render_factory_parameters(ctx, &externals, has_exports && named_exports);

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
  // TODO indent chunk content
  module_sources.into_iter().for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources {
        concat_source.add_source(source);
      }
    }
  });

  //  exports
  if let Some(exports) = render_chunk_exports(ctx, Some(&export_mode)) {
    concat_source.add_source(Box::new(RawSource::new(exports)));
  }

  if let Some(outro) = outro {
    concat_source.add_source(Box::new(RawSource::new(outro)));
  }

  // umd wrapper end
  concat_source.add_source(Box::new(RawSource::new(format!("}});"))));

  if let Some(footer) = footer {
    concat_source.add_source(Box::new(RawSource::new(footer)));
  }

  Ok(concat_source)
}

fn render_amd_dependencies(externals: &[ExternalRenderImportStmt], has_exports: bool) -> String {
  let mut dependencies = if has_exports { vec!["exports"] } else { vec![] };
  externals.iter().for_each(|external| {
    dependencies.push(&external.path.as_str());
  });
  dependencies.join(", ")
}

fn render_cjs_dependencies(externals: &[ExternalRenderImportStmt], has_exports: bool) -> String {
  let mut dependencies = if has_exports { vec!["exports".to_string()] } else { vec![] };
  externals.iter().for_each(|external| {
    dependencies.push(format!("require('{}')", external.path.as_str()));
  });
  dependencies.join(", ")
}

fn render_global_dependencies(
  ctx: &mut GenerateContext<'_>,
  externals: &[ExternalRenderImportStmt],
  has_exports: bool,
  export_mode: &OutputExports,
) -> DiagnosableResult<String> {
  let mut dependencies = if has_exports {
    if ctx.options.name.as_ref().map_or(true, String::is_empty)
      && !matches!(export_mode, OutputExports::None)
    {
      return Err(vec![BuildDiagnostic::missing_name_option_for_umd_export()]);
    }
    vec!["exports".to_string()]
  } else {
    vec![]
  };
  externals.iter().for_each(|external| {
    dependencies.push(format!("require('{}')", external.path.as_str()));
  });
  Ok(dependencies.join(", "))
}

fn generate_namespace_definition(name: &str) -> (String, String) {
  let mut initial_code = String::new();
  let mut final_code = String::from("this");

  let context_len = final_code.len();
  let parts: Vec<&str> = name.split('.').collect();

  for (i, part) in parts.iter().enumerate() {
    let caller = generate_caller(part);
    final_code.push_str(&caller);

    if i < parts.len() - 1 {
      let callers = &final_code[context_len..];
      initial_code.push_str(&format!("this{callers} = this{callers} || {{}};\n"));
    }
  }

  (initial_code, final_code)
}
