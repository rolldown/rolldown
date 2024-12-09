use crate::utils::chunk::determine_export_mode::determine_export_mode;
use crate::utils::chunk::namespace_marker::render_namespace_markers;
use crate::utils::chunk::render_chunk_exports::get_export_items;
use crate::{
  ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::{
    determine_use_strict::determine_use_strict, render_chunk_exports::render_chunk_exports,
  },
};
use rolldown_common::{ExportsKind, OutputExports, WrapKind};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::concat_string;

use super::utils::render_modules_with_peek_runtime_module_at_first;

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub fn render_cjs<'code>(
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

  if determine_use_strict(ctx) {
    source_joiner.append_source("\"use strict\";");
  }

  if let Some(intro) = intro {
    source_joiner.append_source(intro);
  }

  // Note that the determined `export_mode` should be used in `render_chunk_exports` to render exports.
  // We also need to get the export mode for rendering the namespace markers.
  // So we determine the export mode (from auto) here and use it in the following code.
  let export_mode = if let Some(entry_module) =
    ctx.chunk.user_defined_entry_module(&ctx.link_output.module_table)
  {
    if matches!(entry_module.exports_kind, ExportsKind::Esm) {
      let export_items = get_export_items(ctx.chunk, ctx.link_output);
      let has_default_export = export_items.iter().any(|(name, _)| name.as_str() == "default");
      let export_mode = determine_export_mode(warnings, ctx, entry_module, &export_items)?;
      // Only `named` export can we render the namespace markers.
      if matches!(&export_mode, OutputExports::Named) {
        if let Some(marker) =
          render_namespace_markers(ctx.options.es_module, has_default_export, false)
        {
          source_joiner.append_source(marker.to_string());
        }
      }
      Some(export_mode)
    } else {
      // There is no need for a non-ESM export kind for determining the export mode.
      None
    }
  } else {
    // No need for common chunks to determine the export mode.
    None
  };

  // Runtime module should be placed before the generated `requires` in CJS format.
  // Because, we might need to generate `__toESM(require(...))` that relies on the runtime module.
  render_modules_with_peek_runtime_module_at_first(
    ctx,
    &mut source_joiner,
    module_sources,
    render_cjs_chunk_imports(ctx),
  );

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

        // module.exports = require_xxx();
        source_joiner.append_source(concat_string!("module.exports = ", wrapper_ref_name, "();\n"));
      }
      WrapKind::None => {}
    }
  }

  let export_mode = export_mode.unwrap_or(OutputExports::Auto);

  if let Some(exports) = render_chunk_exports(ctx, Some(&export_mode)) {
    source_joiner.append_source(exports);
  }

  if let Some(outro) = outro {
    source_joiner.append_source(outro);
  }

  if let Some(footer) = footer {
    source_joiner.append_source(footer);
  }

  Ok(source_joiner)
}

// Make sure the imports generate stmts keep live bindings.
fn render_cjs_chunk_imports(ctx: &GenerateContext<'_>) -> String {
  let mut s = String::new();

  // render imports from other chunks
  ctx.chunk.imports_from_other_chunks.iter().for_each(|(exporter_id, items)| {
    let importee_chunk = &ctx.chunk_graph.chunk_table[*exporter_id];
    let require_path_str =
      concat_string!("require('", ctx.chunk.import_path_for(importee_chunk), "');\n");
    if items.is_empty() {
      s.push_str(&require_path_str);
    } else {
      s.push_str("const ");
      s.push_str(&ctx.chunk.require_binding_names_for_other_chunks[exporter_id]);
      s.push_str(" = ");
      s.push_str(&require_path_str);
    }
  });

  // render external imports
  ctx.chunk.imports_from_external_modules.iter().for_each(|(importee_id, _)| {
    let importee = ctx.link_output.module_table.modules[*importee_id]
      .as_external()
      .expect("Should be external module here");

    let require_path_str = concat_string!("require(\"", &importee.name, "\")");

    if ctx.link_output.used_symbol_refs.contains(&importee.namespace_ref) {
      let to_esm_fn_name = ctx.finalized_string_pattern_for_symbol_ref(
        ctx.link_output.runtime.resolve_symbol("__toESM"),
        ctx.chunk_idx,
        &ctx.chunk.canonical_names,
      );

      let external_module_symbol_name = &ctx.chunk.canonical_names[&importee.namespace_ref];
      s.push_str("const ");
      s.push_str(external_module_symbol_name);
      s.push_str(" = ");
      s.push_str(&to_esm_fn_name);
      s.push('(');
      s.push_str(&require_path_str);
      s.push_str(");\n");
    } else if importee.side_effects.has_side_effects() {
      s.push_str(&require_path_str);
      s.push_str(";\n");
    }
  });

  s
}
